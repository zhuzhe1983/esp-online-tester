
#include "command_processor.hpp"
#include <esp_log.h>

void setup(void)
{
  command_processor::setup();
}

void loop(void)
{
  command_processor::loop();
}

#if !defined ( ARDUINO )

#include <freertos/FreeRTOS.h>
#include <freertos/task.h>

extern "C" {
  void loopTask(void*)
  {
    setup();
    for (;;) {
      loop();
      taskYIELD();
    }
    vTaskDelete(NULL);
  }

  void app_main()
  {
    xTaskCreatePinnedToCore( loopTask
                           , "loopTask"
                           , 8192
                           , NULL
                           , 1
                           , NULL
                           , 1);
  }
}
#endif
