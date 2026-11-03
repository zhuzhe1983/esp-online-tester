idf_component_register(SRCS "main.c" INCLUDE_DIRS "." REQUIRES driver esp_driver_gpio esp_driver_spi esp_driver_uart)
if(STAMP_C6LORA_QUEUED_SPI)
  target_compile_definitions(${COMPONENT_LIB} PRIVATE STAMP_C6LORA_QUEUED_SPI=1)
endif()
