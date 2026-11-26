//! Copyright (c) M5Stack. All rights reserved.
//! Licensed under the MIT license. See LICENSE file in the project root for full license information.

#include "common.hpp"
#include "command_processor.hpp"
#include "i2c_slave.hpp"
#include "update.hpp"

#include <cstring>
#include <driver/gpio.h>
#include <driver/i2c.h>
#include <driver/periph_ctrl.h>
#include <driver/spi_master.h>
#include <esp_log.h>
#include <esp_sleep.h>
#include <freertos/FreeRTOS.h>
#include <freertos/task.h>
#include <hal/gpio_types.h>
#include <nvs_flash.h>
#include <nvs.h>
#include <rom/ets_sys.h>
#include <sdkconfig.h>
#include <soc/gpio_periph.h>
#include <soc/i2c_periph.h>
#include <soc/i2c_reg.h>
#include <soc/i2c_struct.h>
#include <soc/rtc.h>
#include <soc/rtc_cntl_reg.h>
#include <soc/soc.h>
#include <soc/spi_reg.h>
#include <soc/uart_reg.h>

#include <algorithm>

namespace command_processor
{
  static constexpr char NVS_KEY_REGDATA[] = "REGDATA";
  static constexpr uint8_t I2C_ADDR_DEFAULT = 0x66;  // I2C address
  static constexpr gpio_num_t PIN_LED  = GPIO_NUM_1;
  static constexpr gpio_num_t PIN_SCL  = GPIO_NUM_3;
  static constexpr gpio_num_t PIN_SDA  = GPIO_NUM_4;
  static constexpr gpio_num_t PIN_MISO = GPIO_NUM_5;
  static constexpr gpio_num_t PIN_SCLK = GPIO_NUM_6;
  static constexpr gpio_num_t PIN_CS   = GPIO_NUM_7;
  static constexpr spi_host_device_t SPI_HOST = SPI2_HOST;
  static constexpr i2c_port_t I2C_PORT = (i2c_port_t)I2C_NUM_0;
  static spi_device_handle_t spi_handle = nullptr;
  static uint32_t read_reg_index = 0;
  static int32_t write_reg_index = -1;
  static bool reg_changed = false;
  static bool cmd_changed = false;
  static constexpr size_t register_storage_size = 16;
  static constexpr size_t register_virtual_size = 17;
  static uint8_t register_data[64];
  static constexpr uint8_t I2C_MIN_ADDR = 0x08;
  static constexpr uint8_t I2C_MAX_ADDR = 0x77;

  static constexpr std::size_t RX_BUFFER_MAX = 0x10;
  static constexpr std::size_t PARAM_MAXLEN = 12;
  volatile std::size_t _rx_buffer_setpos = 0;
  volatile std::size_t _rx_buffer_getpos = 0;
  std::uint8_t _rx_buffer[RX_BUFFER_MAX][PARAM_MAXLEN];
  std::uint8_t* _params = _rx_buffer[0];
  std::size_t _param_index = 0;
  std::size_t _param_need_count = 1;
  std::size_t _param_resetindex = 0;
  enum firmupdate_state_t
  {
    nothing ,       // コマンド未受信;
    wait_data ,     // データ待機;
    progress ,      // データ受信中;
    sector_write ,  // セクタブロックのフラッシュ書き込み;
    finish ,        // 全行程終了;
  };
  firmupdate_state_t _firmupdate_state = nothing;
  std::size_t _firmupdate_index = 0;
  std::size_t _firmupdate_totalsize = 0;
  std::size_t _firmupdate_result = 0;
  std::size_t _last_command = 0;

  static constexpr uint8_t CMD_REG_ACCESS   = 0x00;
  static constexpr uint8_t CMD_REG_COMMAND  = 0x10;
  static constexpr uint8_t CMD_UPDATE_BEGIN = 0xF0; // 4Byte ファームウェア更新開始   [0]=0xF0 [1]=0x66 [2]=0x75 [3]=0xF0
  static constexpr uint8_t CMD_UPDATE_DATA  = 0xF1; // 4Byte ファームウェアデータ送信 [0]=0xF1 [1]=0x66 [2]=0x75 [3]=0xF1
  static constexpr uint8_t CMD_UPDATE_END   = 0xF2; // 4Byte ファームウェア更新完了   [0]=0xF2 [1]=0x66 [2]=0x75 [3]=0xF2
  static constexpr uint8_t CMD_RESET        = 0xFF; // 4Byte リセット(再起動)         [0]=0xFF [1]=0x66 [2]=0x75 [3]=0xFF
  static constexpr uint8_t UPDATE_RESULT_BROKEN = 0x01;
  static constexpr uint8_t UPDATE_RESULT_ERROR  = 0x00;
  static constexpr uint8_t UPDATE_RESULT_OK     = 0xF1;
  static constexpr uint8_t UPDATE_RESULT_BUSY   = 0xFF;

  static constexpr uint8_t SLEEP_COMMAND_WAKE_TIMER = 0xEE;
  static constexpr uint8_t SLEEP_COMMAND_WAKE_TIMER_AND_I2C = 0xEF;

  // register list
  // index : 0x00 - 0x03 = (read ) temperature data (MAX6675/MAX31855 SPI read)
  // index : 0x04        = (const) device id ( 0x31 )
  // index : 0x05        = (const) device id ( 0x85 )
  // index : 0x06        = (const) major version
  // index : 0x07        = (const) minor version
  // index : 0x08        = I2C addr
  // index : 0x09        = I2C addr (bit invert)

  // index : 0x0E        = Sleep time in seconds information (16bit high byte)
  // index : 0x0F        = Sleep time in seconds information (16bit low byte)
  // index : 0x10        = sleep command write.
  //                       Write 0xEE here to DeepSleep (wake up timer)
  //                       Write 0xEF,here to DeepSleep (wake up timer and I2C_SCL low wake up)

  static constexpr uint8_t REGINDEX_WRITEABLE = 0x08;
  static constexpr uint8_t REGINDEX_I2C_ADDR = 0x08;
  static constexpr uint8_t REGINDEX_I2C_ADDR_INV = 0x09;
  static constexpr uint8_t REGINDEX_SLEEP_SEC_H = 0x0E;
  static constexpr uint8_t REGINDEX_SLEEP_SEC_L = 0x0F;

  static inline volatile uint32_t* get_gpio_hi_reg(int_fast8_t pin) { return &GPIO.out_w1ts.val; }
  static inline volatile uint32_t* get_gpio_lo_reg(int_fast8_t pin) { return &GPIO.out_w1tc.val; }
  static inline void gpio_hi(int_fast8_t pin) { *get_gpio_hi_reg(pin) = 1 << (pin & 31); }
  static inline void gpio_lo(int_fast8_t pin) { *get_gpio_lo_reg(pin) = 1 << (pin & 31); }

  #define REG_SPI_BASE(i)     (DR_REG_SPI2_BASE)
  static constexpr uint32_t SPI_EXECUTE = SPI_USR | SPI_UPDATE;
  #define SPI_MISO_DLEN_REG(i) (REG_SPI_BASE(i) + 0x1C)

  static IRAM_ATTR void logRegData(void)
  {
    ESP_EARLY_LOGV(LOGNAME, "reg: %02x %02x %02x %02x %02x %02x %02x %02x %02x %02x %02x %02x %02x %02x %02x %02x"
            , register_data[0x00], register_data[0x01], register_data[0x02], register_data[0x03]
            , register_data[0x04], register_data[0x05], register_data[0x06], register_data[0x07]
            , register_data[0x08], register_data[0x09], register_data[0x0A], register_data[0x0B]
            , register_data[0x0C], register_data[0x0D], register_data[0x0E], register_data[0x0F]
            );
  }

  static void IRAM_ATTR spi_read_task(void*)
  {
    setActive(proc_spi);
    spi_bus_config_t buscfg = {
        .mosi_io_num = -1,
        .miso_io_num = PIN_MISO,
        .sclk_io_num = PIN_SCLK,
        .quadwp_io_num = -1,
        .quadhd_io_num = -1,
        .max_transfer_sz = 1,
        .flags = SPICOMMON_BUSFLAG_MASTER,
        .intr_flags = 0,
    };
    if (ESP_OK != spi_bus_initialize(static_cast<spi_host_device_t>(SPI_HOST), &buscfg, spi_common_dma_t::SPI_DMA_CH_AUTO))
    {
      ESP_EARLY_LOGE(LOGNAME, "Failed to spi_bus_initialize. ");
    }

    spi_device_interface_config_t devcfg = {
        .command_bits = 0,
        .address_bits = 0,
        .dummy_bits = 0,
        .mode = 0,
        .duty_cycle_pos = 0,
        .cs_ena_pretrans = 0,
        .cs_ena_posttrans = 0,
        .clock_speed_hz = 1000000,
        .input_delay_ns = 0,
        .spics_io_num = PIN_CS,
        .flags = 0,
        .queue_size = 1,
        .pre_cb = nullptr,
        .post_cb = nullptr};
    if (ESP_OK != spi_bus_add_device(SPI_HOST, &devcfg, &spi_handle)) {
      ESP_EARLY_LOGE(LOGNAME, "Failed to spi_bus_add_device. ");
    }

    spi_device_acquire_bus(spi_handle, portMAX_DELAY);

    uint32_t spi_port = (SPI_HOST + 1);
    (void)spi_port;

    WRITE_PERI_REG(SPI_MISO_DLEN_REG(spi_port), (4 << 3) - 1); // 4 Byte read setting

    for (;;)
    {
      if (_firmupdate_state == firmupdate_state_t::nothing) {
        gpio_lo(PIN_CS);
        WRITE_PERI_REG(SPI_CLK_GATE_REG(spi_port), SPI_CLK_EN | SPI_MST_CLK_ACTIVE);
        WRITE_PERI_REG(SPI_CMD_REG(spi_port), SPI_EXECUTE);

        do { taskYIELD(); } while (READ_PERI_REG(SPI_CMD_REG(spi_port)) & SPI_USR);

        *(uint32_t*)register_data = READ_PERI_REG(SPI_W0_REG(spi_port));
        gpio_hi(PIN_CS);
        WRITE_PERI_REG(SPI_CLK_GATE_REG(spi_port), 0);
#if CORE_DEBUG_LEVEL > 0
        ESP_EARLY_LOGV(LOGNAME, "SENSOR DATA %02x %02x %02x %02x", register_data[0], register_data[1], register_data[2], register_data[3]);
#endif
      }
      uint32_t delaytime = 200 / portTICK_PERIOD_MS;
      if (register_data[1] & 1 || register_data[3] & 7) {
        delaytime = 10 / portTICK_PERIOD_MS;
#if CORE_DEBUG_LEVEL > 0
        ESP_EARLY_LOGI(LOGNAME, "SENSOR ERROR %02x %02x %02x %02x", register_data[0], register_data[1], register_data[2], register_data[3]);
#endif
        continue;
      }
      setDeactive(proc_spi);
      vTaskDelay(delaytime);
      setActive(proc_spi);
    }
  }

  static void IRAM_ATTR save_nvs(void)
  {
    uint32_t handle = 0;
    if (ESP_OK != nvs_open(LOGNAME, NVS_READWRITE, &handle))
    {
      ESP_EARLY_LOGI(LOGNAME, "nvs open error.");
    }
    else
    {
      ESP_EARLY_LOGI(LOGNAME, "nvs save");
      nvs_set_blob(handle, NVS_KEY_REGDATA, &register_data[REGINDEX_WRITEABLE], register_storage_size - REGINDEX_WRITEABLE);

      nvs_commit(handle);
      nvs_close(handle);
    }
    logRegData();
  }

  static void IRAM_ATTR load_nvs(void)
  {
    ESP_EARLY_LOGI(LOGNAME, "load_nvs");
    memset(&register_data[REGINDEX_WRITEABLE], 0, sizeof(register_data) - REGINDEX_WRITEABLE);
    uint32_t handle;
    esp_err_t init = nvs_flash_init();
    if (ESP_OK != nvs_open(LOGNAME, NVS_READONLY, &handle))
    {
      ESP_EARLY_LOGI(LOGNAME, "nvs open error.  start nvs erase ...");
      while ( init != ESP_OK )
      {
        taskYIELD();
        nvs_flash_erase();
        init = nvs_flash_init();
      }
      ESP_EARLY_LOGI(LOGNAME, "done.");

      register_data[REGINDEX_I2C_ADDR    ] =  I2C_ADDR_DEFAULT;
      register_data[REGINDEX_I2C_ADDR_INV] = ~I2C_ADDR_DEFAULT;

      save_nvs();
    }
    else
    {
      size_t len = register_storage_size - REGINDEX_WRITEABLE;
      if (ESP_OK != nvs_get_blob(handle, NVS_KEY_REGDATA, &register_data[REGINDEX_WRITEABLE], &len))
      {
        ESP_EARLY_LOGE(LOGNAME, "nvs error: can't get address.");
      }
      nvs_close(handle);
    }
    register_data[4] = DEVICE_ID_0; // Device ID
    register_data[5] = DEVICE_ID_1; // Device ID
    register_data[6] = FIRMWARE_MAJOR_VERSION; // major ver
    register_data[7] = FIRMWARE_MINOR_VERSION; // minor ver
    logRegData();
  }


  static bool IRAM_ATTR command(void)
  {
    if (_rx_buffer_getpos == _rx_buffer_setpos)
    {
      return false;
    }
    const std::uint8_t* params = _rx_buffer[_rx_buffer_getpos];

  #if DEBUG == 1
    if (cmd_detect[params[0]] == 0)
    {
      cmd_detect[params[0]] = 1;
      ESP_EARLY_LOGI(LOGNAME, "CMD:%02x", params[0]);
    }
  #endif

    switch (params[0])
    {
    default:
      ESP_EARLY_LOGI(LOGNAME, "unknown CMD:%02x", params[0]);
      break;

    case CMD_UPDATE_BEGIN:
      update::initCRCtable();
      break;

    case CMD_UPDATE_DATA:
      _firmupdate_result = UPDATE_RESULT_BUSY;
      ESP_EARLY_LOGD(LOGNAME, "flash:%d", _firmupdate_index);
      if (!update::writeBuffer(_firmupdate_index))
      {
        ESP_EARLY_LOGE(LOGNAME, "OTA write fail");
        _firmupdate_result = UPDATE_RESULT_ERROR;
      }
      else
      {
        _firmupdate_result = UPDATE_RESULT_OK;
      }
      i2c_slave::set_txdata(_firmupdate_result, 8);

      _firmupdate_state = firmupdate_state_t::wait_data;
      _firmupdate_index += SPI_FLASH_SEC_SIZE;
      break;

    case CMD_UPDATE_END:
      if (update::end())
      {
        ESP_EARLY_LOGI(LOGNAME, "success! rebooting...");
        esp_restart();
      }
      else
      {
        ESP_EARLY_LOGE(LOGNAME, "OTA close fail");
      }
      break;
    }

    _rx_buffer_getpos = (_rx_buffer_getpos + 1) & (RX_BUFFER_MAX - 1);
    return true;
  }

  void setup(void)
  {
    rtc_clk_cpu_freq_set_xtal();
    setActive(proc_main);

    gpio_config_t io_conf;
    io_conf.mode         = GPIO_MODE_DISABLE;
    io_conf.intr_type    = GPIO_INTR_DISABLE;
    io_conf.pull_down_en = GPIO_PULLDOWN_DISABLE;
    io_conf.pull_up_en   = GPIO_PULLUP_DISABLE;
    io_conf.pin_bit_mask = 1 << GPIO_NUM_0
                         | 1 << GPIO_NUM_1
                         | 1 << GPIO_NUM_2
                         | 1 << GPIO_NUM_3
                         | 1 << GPIO_NUM_4
                         | 1 << GPIO_NUM_5
                         | 1 << GPIO_NUM_6
                         | 1 << GPIO_NUM_7
                         | 1 << GPIO_NUM_8
                         | 1 << GPIO_NUM_9
                         | 1 << GPIO_NUM_10
                         | 1 << GPIO_NUM_11
                         | 1 << GPIO_NUM_18
                         | 1 << GPIO_NUM_19
#if CORE_DEBUG_LEVEL == 0
                         | 1 << GPIO_NUM_20
                         | 1 << GPIO_NUM_21
#endif
                         ;
    gpio_config(&io_conf);

    memset(register_data, 0xFF, 4);
    xTaskCreate(spi_read_task, "spi_read_task", 2048, nullptr, 1, nullptr);

#if CORE_DEBUG_LEVEL > 0
    io_conf.pin_bit_mask = (uint64_t)1 << PIN_LED;
    io_conf.mode         = GPIO_MODE_OUTPUT;
    gpio_config(&io_conf);
    gpio_lo(PIN_LED);
#else
    periph_module_disable(periph_module_t::PERIPH_UART0_MODULE);
    periph_module_disable(periph_module_t::PERIPH_UART1_MODULE);
#endif

    load_nvs();

    uint32_t addr = I2C_ADDR_DEFAULT;
    if (register_data[REGINDEX_I2C_ADDR] == (uint8_t)(~register_data[REGINDEX_I2C_ADDR_INV])
      && register_data[REGINDEX_I2C_ADDR] >= I2C_MIN_ADDR
      && register_data[REGINDEX_I2C_ADDR] <= I2C_MAX_ADDR
      )
    {
      addr = register_data[REGINDEX_I2C_ADDR];
    }

    i2c_slave::init(I2C_PORT, PIN_SDA, PIN_SCL, addr, xTaskGetCurrentTaskHandle());
  }

  static union {
    uint32_t active_procs_raw = 0;
    uint8_t active_procs[proc_max];
  };

  void IRAM_ATTR setActive(proc_t proc)
  {
    active_procs[proc] = 1;
    ets_update_cpu_frequency(RTC_XTAL_FREQ_40M);
    REG_SET_FIELD(SYSTEM_SYSCLK_CONF_REG, SYSTEM_PRE_DIV_CNT, 0);
    rtc_clk_apb_freq_update(RTC_XTAL_FREQ_40M * MHZ);
  }

  void IRAM_ATTR setDeactive(proc_t proc)
  {
    static constexpr uint32_t SLOW_FREQ_MHZ = 10;
    active_procs[proc] = 0;
    if (active_procs_raw) {
      return;
    }

    ets_update_cpu_frequency(SLOW_FREQ_MHZ);
    REG_SET_FIELD(SYSTEM_SYSCLK_CONF_REG, SYSTEM_PRE_DIV_CNT, (RTC_XTAL_FREQ_40M / SLOW_FREQ_MHZ) - 1);
    rtc_clk_apb_freq_update(SLOW_FREQ_MHZ * MHZ);
  }

  void IRAM_ATTR loop(void)
  {
#if CORE_DEBUG_LEVEL > 0
    static bool led;
    led = !led;
    auto gpio_reg = led ? get_gpio_hi_reg(PIN_LED) : get_gpio_lo_reg(PIN_LED);
    *gpio_reg = 1 << (PIN_LED & 31);
#endif

    setDeactive(proc_main);
    vTaskDelay(2);
    setActive(proc_main);

    command();

    if (_firmupdate_state == firmupdate_state_t::nothing && !i2c_slave::is_busy())
    {
      if (reg_changed)
      {
        reg_changed = false;
        save_nvs();
      }

      if (cmd_changed)
      {
        cmd_changed = false;
        auto regcmd = register_data[CMD_REG_COMMAND];
        register_data[CMD_REG_COMMAND] = 0;
        if (regcmd) {
          switch (regcmd) {
          default:
            break;
          case SLEEP_COMMAND_WAKE_TIMER_AND_I2C:
/// SCL LOW wakeup
            esp_deep_sleep_enable_gpio_wakeup(1 << PIN_SCL, esp_deepsleep_gpio_wake_up_mode_t::ESP_GPIO_WAKEUP_GPIO_LOW);
            esp_sleep_enable_gpio_wakeup();

            [[fallthrough]];

          case SLEEP_COMMAND_WAKE_TIMER:
            {
              uint32_t sec = (register_data[REGINDEX_SLEEP_SEC_H] << 8 | register_data[REGINDEX_SLEEP_SEC_L]);
#if CORE_DEBUG_LEVEL > 0
              ESP_EARLY_LOGI(LOGNAME, "SLEEP:%d sec", sec);
              vTaskDelay(1);
#endif
                // Shorten 300 msec as required time for startup and initial data acquisition.
              uint64_t us = sec ? 1000ull * (sec * 1000 - 300) : 0ull;
              esp_sleep_enable_timer_wakeup(us);
              esp_deep_sleep_start();
            }
            break;
          }
        }
      }
    }
  }

  /// I2C STOP時などのデータの区切りの処理;
  void IRAM_ATTR closeData(void)
  {
    _param_index = 0;
    _param_need_count = 1;
    _param_resetindex = 0;
  }

  /// I2CペリフェラルISRから1Byteずつデータを受取る処理;
  bool IRAM_ATTR addData(std::uint8_t value)
  {
// if (_param_index == 0)
// ESP_EARLY_LOGV(LOGNAME, "n:%d : v:%02x", _param_need_count, value);
    _params[_param_index] = value;

    if (++_param_index == 1)
    {
      _last_command = value;
      _param_resetindex = 0;
// ESP_EARLY_LOGD(LOGNAME, "CMD:%02x", value);
      switch (value)
      {
      default:
        value &= 31;
        i2c_slave::set_txdata(&register_data[value], 32);
        read_reg_index  = value + 32;
        write_reg_index = value;
        _params[0] = CMD_REG_ACCESS;
        _last_command = CMD_REG_ACCESS;
        _param_need_count = 2;
        _param_resetindex = 1;
        return false;

      case CMD_UPDATE_END:
        _param_need_count = 4;
        return false;

      case CMD_UPDATE_BEGIN:
        _param_need_count = 8;
        return false;

      case CMD_UPDATE_DATA:
        _param_need_count = 8;
        return false;
      }
    }

    if (_param_index >= _param_need_count)
    {
      switch (_params[0])
      {
      default:
        break;

      case CMD_REG_ACCESS:
        {
          int wridx = write_reg_index;
          if (wridx < register_virtual_size)
          {
            uint8_t val = _params[1];
            if (register_data[wridx] != val)
            {
              register_data[wridx] = val;
              reg_changed |= wridx < register_storage_size;
            }
            cmd_changed |= wridx >= register_storage_size;
          }
          write_reg_index = wridx + 1;
          --_param_index;
        }
        return reg_changed || cmd_changed;

      /// ファームウェアアップデートの準備コマンド;
      case CMD_UPDATE_BEGIN:
        if ((_params[1] == DEVICE_ID_0)
         && (_params[2] == DEVICE_ID_1)
         && (_params[0] == _params[3])
        )
        {
          setActive(proc_updater);

          _firmupdate_state = firmupdate_state_t::wait_data;
          _firmupdate_index = 0;
          _firmupdate_result = UPDATE_RESULT_ERROR; /// 途中中断した時のためリード応答にはエラーステートを設定しておく;
          _firmupdate_totalsize = _params[4] << 24 | _params[5] << 16 | _params[6] << 8 | _params[7];
          update::begin(_firmupdate_totalsize);
          i2c_slave::set_txdata(_firmupdate_result, 8);
        }
        else
        {
          closeData();
          return false;
        }
        break;

      /// ファームウェアアップデートのデータ受信コマンド;
      case CMD_UPDATE_DATA:
        if (_firmupdate_state == firmupdate_state_t::progress)
        {
          /// 受信したデータをupdateに蓄積;
          if (update::addData(_params[1]))
          {
            _param_index = _param_resetindex;
            return false;
          }

          if (update::checkCRC32())
          {
            _firmupdate_result = UPDATE_RESULT_BUSY;
          }
          else
          {
            _firmupdate_result = UPDATE_RESULT_BROKEN;
          }
          i2c_slave::set_txdata(_firmupdate_result, 8);
          _firmupdate_state = sector_write;
          closeData();
        }
        else
        if ((_params[1] == DEVICE_ID_0)
         && (_params[2] == DEVICE_ID_1)
         && (_params[0] == _params[3])
        )
        {
          update::setBlockCRC32( _params[4] << 24 | _params[5] << 16 | _params[6] << 8 | _params[7] );
          _param_need_count = 2;
          _param_resetindex = 1;
          _param_index = _param_resetindex;
          _firmupdate_state = firmupdate_state_t::progress;
          _firmupdate_result = UPDATE_RESULT_ERROR; /// 途中中断した時のためリード応答にはエラーステートを設定しておく;
          return false;
        }
        else
        {
          closeData();
          return false;
        }
        break;
      }

      auto new_setpos = (_rx_buffer_setpos + 1) & (RX_BUFFER_MAX - 1);
      _rx_buffer_setpos = new_setpos;
      _param_index = _param_resetindex;
      for (std::size_t i = 0; i < _param_resetindex; ++i)
      {
        _rx_buffer[new_setpos][i] = _params[i];
      }
      _params = _rx_buffer[new_setpos];

      return true;
    }
    return false;
  }

  void IRAM_ATTR prepareTxData(void)
  {
    switch (_last_command)
    {
    case CMD_REG_ACCESS:
      if (read_reg_index + 8 < register_storage_size)
      {
        i2c_slave::add_txdata(&register_data[read_reg_index], 8);
        read_reg_index += 8;
      }
      else {
        for (std::size_t i = 0; i < 16; ++i)
        {
          i2c_slave::add_txdata((read_reg_index < register_storage_size)
                              ? register_data[read_reg_index]
                              : 0);
          ++read_reg_index;
        }
      }
      break;

    case CMD_UPDATE_DATA:
      i2c_slave::add_txdata(_firmupdate_result);
      break;

    default:
      {
        std::uint8_t dummy[] = { 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff };
        i2c_slave::add_txdata(dummy, 8);
      }
      break;
    }
  }

}
