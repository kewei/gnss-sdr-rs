#include "../include/rtl_sdr_wrapper.h"
#include <stdlib.h>

// C callback function
void c_callback_read_buffer(uint8_t* data, uint32_t length);

// Wrapper for rtlsdr_read_async with C callback
int rtl_sdr_read_async_wrapper(rtlsdr_dev_t *dev,
				 uint32_t buff_num,
				 uint32_t buff_len) {
    return rtlsdr_read_async(dev,
				 c_callback_read_buffer,
                 NULL,
				 buff_num,
				 buff_len);
}
