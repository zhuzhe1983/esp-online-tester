/*
 * SPDX-FileCopyrightText: 2024 M5Stack Technology CO LTD
 *
 * SPDX-License-Identifier: MIT
 */
/*!
  @file unit_BMP280_data.hpp
  @brief BMP280 measurement data, trimming parameters, and compensation (framework independent)
*/
#ifndef M5_UNIT_ENV_UNIT_BMP280_DATA_HPP
#define M5_UNIT_ENV_UNIT_BMP280_DATA_HPP

#include <array>
#include <cstdint>
#include <limits>  // NaN

namespace m5 {
namespace unit {
namespace bmp280 {

/*!
  @union Trimming
  @brief Trimming parameter
*/
union Trimming {
    uint8_t value[12 * 2]{};
    struct {
        //
        uint16_t dig_T1;
        int16_t dig_T2;
        int16_t dig_T3;
        //
        uint16_t dig_P1;
        int16_t dig_P2;
        int16_t dig_P3;
        int16_t dig_P4;
        int16_t dig_P5;
        int16_t dig_P6;
        int16_t dig_P7;
        int16_t dig_P8;
        int16_t dig_P9;
        // uint16_t reserved;
    } __attribute__((packed));
};

///@cond
// Raw ADC value when the measurement is skipped (osrs Skipped)
constexpr uint32_t NOT_MEASURED{0x800000};

// Bosch compensation formulas. t_fine couples pressure to temperature: pressure() runs the
// temperature compensation first (on a fresh instance) to obtain t_fine.
struct Calculator {
    inline float temperature(const int32_t adc_P, const int32_t adc_T, const Trimming* t)
    {
        return t ? compensate_temperature_f(adc_T, *t) : std::numeric_limits<float>::quiet_NaN();
    }
    inline float pressure(const int32_t adc_P, const int32_t adc_T, const Trimming* t)
    {
        if (!t) {
            return std::numeric_limits<float>::quiet_NaN();
        }
        if (t_fine == 0) {
            (void)compensate_temperature_f(adc_T, *t);  // For t_fine
        }
        return compensate_pressure_f(adc_P, *t);
    }

private:
    float compensate_temperature(const int32_t adc_T, const Trimming& trim)
    {
        int32_t var1{}, var2{};
        var1 = ((((adc_T >> 3) - ((int32_t)trim.dig_T1 << 1))) * ((int32_t)trim.dig_T2)) >> 11;
        var2 = (((((adc_T >> 4) - ((int32_t)trim.dig_T1)) * ((adc_T >> 4) - ((int32_t)trim.dig_T1))) >> 12) *
                ((int32_t)trim.dig_T3)) >>
               14;
        t_fine  = var1 + var2;  // [*1]
        float T = static_cast<float>((t_fine * 5 + 128) >> 8);
        return T * 0.01f;
    }

    float compensate_pressure(const int32_t adc_P, const Trimming& trim)
    {
        int64_t var1{}, var2{}, p{};
        var1 = ((int64_t)t_fine) - 128000;  // (*1) using it!
        var2 = var1 * var1 * (int64_t)trim.dig_P6;
        var2 = var2 + ((var1 * (int64_t)trim.dig_P5) << 17);
        var2 = var2 + (((int64_t)trim.dig_P4) << 35);
        var1 = ((var1 * var1 * (int64_t)trim.dig_P3) >> 8) + ((var1 * (int64_t)trim.dig_P2) << 12);
        var1 = (((((int64_t)1) << 47) + var1)) * ((int64_t)trim.dig_P1) >> 33;
        if (var1) {
            p    = 1048576 - adc_P;
            p    = (((p << 31) - var2) * 3125) / var1;
            var1 = (((int64_t)trim.dig_P9) * (p >> 13) * (p >> 13)) >> 25;
            var2 = (((int64_t)trim.dig_P8) * p) >> 19;
            p    = ((p + var1 + var2) >> 8) + (((int64_t)trim.dig_P7) << 4);
            return p / 256.f;
        }
        return 0.0f;
    }

    float compensate_temperature_f(const int32_t adc_T, const Trimming& trim)
    {
        float var1{}, var2{}, T{};
        var1 = (((float)adc_T) / 16384.0f - ((float)trim.dig_T1) / 1024.0f) * ((float)trim.dig_T2);
        var2 = ((((float)adc_T) / 131072.0f - ((float)trim.dig_T1) / 8192.0f) *
                (((float)adc_T) / 131072.0f - ((float)trim.dig_T1) / 8192.0f)) *
               ((float)trim.dig_T3);
        t_fine = (int32_t)(var1 + var2);  // [*2]
        T      = (var1 + var2) / 5120.0f;
        return T;
    }

    float compensate_pressure_f(const int32_t adc_P, const Trimming& trim)
    {
        float var1{}, var2{}, P{};
        var1 = ((float)t_fine / 2.0f) - 64000.0f;  // (*2)
        var2 = var1 * var1 * ((float)trim.dig_P6) / 32768.0f;
        var2 = var2 + var1 * ((float)trim.dig_P5) * 2.0f;
        var2 = (var2 / 4.0f) + (((float)trim.dig_P4) * 65536.0f);
        var1 = (((float)trim.dig_P3) * var1 * var1 / 524288.0f + ((float)trim.dig_P2) * var1) / 524288.0f;
        var1 = (1.0f + var1 / 32768.0f) * ((float)trim.dig_P1);
        if (var1 == 0.0f) {
            return 0;
        }
        P    = 1048576.0f - (float)adc_P;
        P    = (P - (var2 / 4096.0f)) * 6250.0f / var1;
        var1 = ((float)trim.dig_P9) * P * P / 2147483648.0f;
        var2 = P * ((float)trim.dig_P8) / 32768.0f;
        P    = P + (var1 + var2 + ((float)trim.dig_P7)) / 16.0f;
        return P;
    }

    ////

    int32_t t_fine{};
};
///@endcond

/*!
  @struct Data
  @brief Measurement data group
 */
struct Data {
    std::array<uint8_t, 6> raw{};  //!< RAW data [0,1,2]:pressure [3,4,5]:temperature
    //! @brief Points to the owning unit's calibration for compensation
    //! @note The calibration is read once in begin() and never changes, so this pointer stays valid for
    //! the unit's lifetime (a nullptr means "not backed by a unit" and yields NaN). Do not keep a copied
    //! Data after the owning unit is destroyed.
    const Trimming* trimming{};

    //! temperature (Celsius)
    inline float temperature() const
    {
        return celsius();
    }
    //! temperature (Celsius)
    inline float celsius() const
    {
        int32_t adc_P = (int32_t)(((uint32_t)raw[0] << 16) | ((uint32_t)raw[1] << 8) | ((uint32_t)raw[2]));
        int32_t adc_T = (int32_t)(((uint32_t)raw[3] << 16) | ((uint32_t)raw[4] << 8) | ((uint32_t)raw[5]));
        Calculator c{};

        // adc_T is NOT_MEASURED if orsr Skipped (Not measured)
        return (adc_T != NOT_MEASURED) ? c.temperature(adc_P >> 4, adc_T >> 4, trimming)
                                       : std::numeric_limits<float>::quiet_NaN();
    }
    //! temperature (Fahrenheit)
    inline float fahrenheit() const
    {
        return celsius() * 9.0f / 5.0f + 32.f;
    }
    //! pressure (Pa)
    inline float pressure() const
    {
        int32_t adc_P = (int32_t)(((uint32_t)raw[0] << 16) | ((uint32_t)raw[1] << 8) | ((uint32_t)raw[2]));
        int32_t adc_T = (int32_t)(((uint32_t)raw[3] << 16) | ((uint32_t)raw[4] << 8) | ((uint32_t)raw[5]));
        Calculator c{};

        // adc_T/P is NOT_MEASURED if orsr Skipped (Not measured)
        return (adc_T != NOT_MEASURED && adc_P != NOT_MEASURED) ? c.pressure(adc_P >> 4, adc_T >> 4, trimming)
                                                                : std::numeric_limits<float>::quiet_NaN();
    }
};

}  // namespace bmp280
}  // namespace unit
}  // namespace m5
#endif
