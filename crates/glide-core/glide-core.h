#ifndef GLIDE_CORE_H
#define GLIDE_CORE_H

#include <stdint.h>

const char *glide_core_version(void);
char *glide_core_transcribe(const uint8_t *audio_bytes, uint32_t audio_len, const char *config_json);
char *glide_core_cleanup(const char *raw_text, const char *config_json);
char *glide_core_fetch_models(const char *config_json);
void glide_core_free_string(char *s);

#endif
