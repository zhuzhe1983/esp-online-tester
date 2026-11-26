//! Copyright (c) M5Stack. All rights reserved.
//! Licensed under the MIT license. See LICENSE file in the project root for full license information.

#include <cstdint>
#include <cstddef>
#include <driver/i2c.h>
#include <driver/rtc_io.h>
//#include <driver/timer.h>
#include <driver/periph_ctrl.h>
#include <soc/i2c_periph.h>
#include <soc/i2c_reg.h>
#include <soc/i2c_struct.h>
#include <soc/periph_defs.h>
#include <soc/rtc.h>
#include <freertos/FreeRTOS.h>
#include <esp_log.h>

#include "command_processor.hpp"
#include "i2c_slave.hpp"

namespace i2c_slave
{
  static constexpr std::size_t soc_i2c_fifo_len = 32;
  static constexpr std::uint32_t i2c_intr_mask = 0x3fff;  /*!< I2C all interrupt bitmap */
  static constexpr std::uint32_t I2C_TXFIFO_EMPTY_THRESH_VAL     = 4;
  static constexpr std::uint32_t I2C_RXFIFO_FULL_THRESH_VAL      = 1;
  static constexpr std::uint32_t I2C_SLAVE_TIMEOUT_DEFAULT     = 0xFFFFF; /* I2C slave timeout value, APB clock cycle number */
  static constexpr std::uint32_t I2C_SLAVE_SDA_SAMPLE_DEFAULT  = 4;       /* I2C slave sample time after scl positive edge default value */
  static constexpr std::uint32_t I2C_SLAVE_SDA_HOLD_DEFAULT    = 4;       /* I2C slave hold time after scl negative edge default value */

  struct i2c_obj_t
  {
    intr_handle_t intr_handle;  // I2C interrupt handle
    i2c_port_t i2c_num;   // I2C port number
    xTaskHandle main_handle = nullptr;
    std::uint32_t addr;   // I2C slave addr
    gpio_num_t sda;
    gpio_num_t scl;
  };

  i2c_obj_t i2c_obj;

  static __attribute__((always_inline)) inline 
  i2c_dev_t* IRAM_ATTR getDev(i2c_port_t num)
  {
    return &I2C0;
  }

  static __attribute__((always_inline)) inline 
  periph_module_t getPeriphModule(i2c_port_t num)
  {
    return PERIPH_I2C0_MODULE;
//  return (num == 0) ? PERIPH_I2C0_MODULE : PERIPH_I2C1_MODULE;
  }

  static __attribute__((always_inline)) inline 
  std::uintptr_t IRAM_ATTR getFifoAddr(i2c_port_t num)
  {
    return (std::uintptr_t)&(I2C0.fifo_data);
  }

  static __attribute__((always_inline)) inline 
  periph_interrput_t getIntrSource(i2c_port_t num)
  {
    return ETS_I2C_EXT0_INTR_SOURCE;
//  return (num == 0) ? ETS_I2C_EXT0_INTR_SOURCE : ETS_I2C_EXT1_INTR_SOURCE;
  }

  bool IRAM_ATTR is_busy(void)
  {
    auto dev = getDev(i2c_obj.i2c_num);
    return dev->sr.bus_busy;
  }

  uint32_t IRAM_ATTR getTxCount(void)
  {
    auto dev = getDev(i2c_obj.i2c_num);
    return dev->sr.tx_fifo_cnt;
  }

  static void IRAM_ATTR i2c_isr_handler(void *arg)
  {
    command_processor::setActive(command_processor::proc_i2c);

    auto p_i2c = (i2c_obj_t*)arg;
    auto dev = getDev(p_i2c->i2c_num);

    typeof(dev->int_status) int_sts;
    int_sts.val = dev->int_status.val;
    dev->int_clr.val = int_sts.val;
    dev->int_ena.byte_trans_done = true;

    if (int_sts.det_start)
    {
      command_processor::closeData();
    }

    std::uint32_t rx_fifo_cnt = dev->sr.rx_fifo_cnt;
    if (rx_fifo_cnt)
    {
      do
      {
        command_processor::addData(dev->fifo_data.val);
      } while (--rx_fifo_cnt);
    }
    if (dev->sr.slave_rw && dev->sr.tx_fifo_cnt <= I2C_TXFIFO_EMPTY_THRESH_VAL)
    {
      command_processor::prepareTxData();
    }

    if (!dev->sr.slave_addressed && (int_sts.val & (I2C_BYTE_TRANS_DONE_INT_ENA | I2C_TRANS_COMPLETE_INT_ENA)))
    { // 別のスレーブとの通信に反応しないよう以後のbyte_trans_doneを抑止する
      // trans_completeの時点で再設定する
      dev->int_ena.byte_trans_done = int_sts.trans_complete;

      if (!dev->int_status.val)
      {
        command_processor::setDeactive(command_processor::proc_i2c);
      }
    }
  }

  void IRAM_ATTR clear_txdata(void)
  {
    auto dev = getDev(i2c_obj.i2c_num);
    dev->fifo_conf.tx_fifo_rst = 1;
    dev->fifo_conf.tx_fifo_rst = 0;
  }

  void IRAM_ATTR add_txdata(std::uint8_t buf)
  {
    uint32_t fifo_addr = getFifoAddr(i2c_obj.i2c_num);
    WRITE_PERI_REG(fifo_addr, buf);
  }

  void IRAM_ATTR add_txdata(const std::uint8_t* buf, std::size_t len)
  {
    uint32_t fifo_addr = getFifoAddr(i2c_obj.i2c_num);
    for (std::size_t i = 0; i < len; ++i)
    {
      WRITE_PERI_REG(fifo_addr, buf[i]);
    }
    auto dev = getDev(i2c_obj.i2c_num);
    }

  void IRAM_ATTR set_txdata(const std::uint8_t* buf, std::size_t len)
  {
    auto dev = getDev(i2c_obj.i2c_num);
    dev->fifo_conf.tx_fifo_rst = 1;
    dev->fifo_conf.tx_fifo_rst = 0;
    uint32_t fifo_addr = getFifoAddr(i2c_obj.i2c_num);
    do {
        WRITE_PERI_REG(fifo_addr, *buf++);
    } while (--len);
  }
  void IRAM_ATTR set_txdata(std::uint8_t data, std::size_t len)
  {
    auto dev = getDev(i2c_obj.i2c_num);
    dev->fifo_conf.tx_fifo_rst = 1;
    dev->fifo_conf.tx_fifo_rst = 0;
    uint32_t fifo_addr = getFifoAddr(i2c_obj.i2c_num);
    do {
        WRITE_PERI_REG(fifo_addr, data);
    } while (--len);
  }

  void IRAM_ATTR start_isr(void)
  {
    auto dev = getDev(i2c_obj.i2c_num);
    dev->int_clr.val = dev->int_raw.val;
// I2C_BYTE_TRANS_DONE_INT_ENA // 通信相手によらず、1Byte転送の完了を検出すると発生
// I2C_DET_START_INT_ENA      // 通信相手によらず、I2C STARTコンディションを検出すると発生
// I2C_TRANS_COMPLETE_INT_ENA // 通信相手によらず、I2C STOPコンディションを検出すると発生
    dev->int_ena.val = I2C_DET_START_INT_ENA
                     | I2C_TRANS_COMPLETE_INT_ENA
                     | I2C_BYTE_TRANS_DONE_INT_ENA
                     ;
  }

  void IRAM_ATTR stop_isr(void)
  {
    auto dev = getDev(i2c_obj.i2c_num);
    dev->int_ena.val = 0;
    dev->int_clr.val = dev->int_raw.val;
  }

  void IRAM_ATTR i2c_periph_start(void)
  {
    stop_isr();

    i2c_set_pin(i2c_obj.i2c_num, i2c_obj.sda, i2c_obj.scl, GPIO_PULLUP_ENABLE, GPIO_PULLUP_ENABLE, I2C_MODE_SLAVE);

    periph_module_enable(getPeriphModule(i2c_obj.i2c_num));

    auto dev = getDev(i2c_obj.i2c_num);

    typeof(dev->ctr) ctrl_reg;
    ctrl_reg.val = 0;
    ctrl_reg.sda_force_out = 1;
    ctrl_reg.scl_force_out = 1;
    ctrl_reg.slv_tx_auto_start_en = 1;
    dev->ctr.val = ctrl_reg.val;

    typeof(dev->fifo_conf) fifo_conf;
    dev->fifo_conf.tx_fifo_rst = 1;
    fifo_conf.val = 0;
    fifo_conf.tx_fifo_wm_thrhd = I2C_TXFIFO_EMPTY_THRESH_VAL;
    fifo_conf.rx_fifo_wm_thrhd = I2C_RXFIFO_FULL_THRESH_VAL;
    dev->fifo_conf.val = fifo_conf.val;

    dev->slave_addr.addr = i2c_obj.addr;
    dev->slave_addr.en_10bit = 0;

    dev->sda_hold.time = I2C_SLAVE_SDA_HOLD_DEFAULT;
    dev->sda_sample.time = I2C_SLAVE_SDA_SAMPLE_DEFAULT;

    dev->scl_stretch_conf.val = 0;

    // dev->timeout.tout = I2C_SLAVE_TIMEOUT_DEFAULT;
    // dev->scl_filter_cfg.en = 1;
    // dev->scl_filter_cfg.thres = 0;
    // dev->sda_filter_cfg.en = 1;
    // dev->sda_filter_cfg.thres = 0;
    dev->timeout.time_out_value = 31;
    dev->timeout.time_out_en = 0;
    dev->filter_cfg.val = 0;
    dev->filter_cfg.scl_en = 1;
    dev->filter_cfg.scl_thres = 0;
    dev->filter_cfg.sda_en = 1;
    dev->filter_cfg.sda_thres = 0;

    dev->ctr.conf_upgate = 1;

    start_isr();
  }

  bool IRAM_ATTR init(int i2c_num, int pin_sda, int pin_scl, std::uint8_t i2c_addr, void* mainHandle)
  {
    i2c_obj.i2c_num = (i2c_port_t)i2c_num;
    i2c_obj.addr = i2c_addr;
    i2c_obj.sda = (gpio_num_t)pin_sda;
    i2c_obj.scl = (gpio_num_t)pin_scl;
    i2c_obj.main_handle = mainHandle;

    if (ESP_OK == esp_intr_alloc( i2c_periph_signal[i2c_num].irq
                        , ESP_INTR_FLAG_IRAM | ESP_INTR_FLAG_LEVEL3
                        , i2c_isr_handler
                        , &i2c_obj
                        , &(i2c_obj.intr_handle)
                        ))
    {
      i2c_periph_start();
      return true;
    }
    return false;
  }

  void IRAM_ATTR reset(void)
  {
    auto mod = getPeriphModule(i2c_obj.i2c_num);
    periph_module_disable(mod);
    i2c_periph_start();
  }

}
