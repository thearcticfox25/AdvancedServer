#ifndef LOG_H
#define LOG_H

#include <Colors.h>
#include <Config.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdint.h>
#include <stdarg.h>
#include <string.h>

#if defined(__unix) || defined(__unix__)
	#define __FILENAME__ (strrchr(__FILE__, '/') ? strrchr(__FILE__, '/') + 1 : __FILE__)
#else
	#define __FILENAME__ (strrchr(__FILE__, '\\') ? strrchr(__FILE__, '\\') + 1 : __FILE__)
#endif

#define DEBUG_TYPE "\x1B[36mDBG \x1B[0m"
#define INFO_TYPE "\x1B[32mINF \x1B[0m"
#define WARN_TYPE "\x1B[33mWRN \x1B[0m"
#define ERROR_TYPE "\x1B[31mERR \x1B[0m"

#define LOG_GRN
#define LOG_GRA
#define LOG_RED
#define LOG_BLU
#define LOG_YLW
#define LOG_PUR
#define LOG_RST

#define Log(type, fmt, ...) log_fmt(fmt, type, __FILENAME__, __LINE__, ##__VA_ARGS__)
#define Debug(fmt, ...) if(g_config.server_config.logging.log_debug) log_fmt(fmt, DEBUG_TYPE, __FILENAME__, __LINE__, ##__VA_ARGS__)
#define Info(fmt, ...) log_fmt(fmt, INFO_TYPE, __FILENAME__, __LINE__, ##__VA_ARGS__)
#define Warn(fmt, ...) log_fmt(fmt, WARN_TYPE, __FILENAME__, __LINE__, ##__VA_ARGS__)
#define Err(fmt, ...)  log_fmt(fmt, ERROR_TYPE, __FILENAME__, __LINE__, ##__VA_ARGS__)
#define RAssert(x) if (!(x)) { Err("RAssert(" #x ") failed!"); return false; }

typedef void (*loghook_t)(const char* type, const char* message);

bool log_init (void);
void log_uninit (void);
void log_hook (loghook_t func);
void log_fmt (const char* fmt, const char* type, const char* file, int line, ...);

#endif
