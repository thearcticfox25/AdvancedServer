#ifndef MODERATION_H
#define MODERATION_H

#include <cJSON.h>
#include <io/Threads.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdbool.h>

#define PLAYER_DATA_DIR "Player_Data/"
#define BANNED_NICKNAMES_FILE PLAYER_DATA_DIR "Banned_Nicknames.json"
#define BANNED_UDIDS_FILE PLAYER_DATA_DIR "Banned_UDIDs.json"
#define BANNED_IPS_FILE PLAYER_DATA_DIR "Banned_IPs.json"
#define OPERATORS_FILE PLAYER_DATA_DIR "Operators.json"
#define WHITELIST_FILE PLAYER_DATA_DIR "Whitelist.json"
#define TIMEOUTS_FILE PLAYER_DATA_DIR "Timeouts.json"

extern cJSON* g_whitelisted_players;
extern cJSON* g_banned_nicknames;
extern cJSON* g_banned_udids;
extern cJSON* g_banned_ips;
extern cJSON* g_timeouts;
extern cJSON* g_ops;
extern Mutex g_banMut;
extern Mutex g_timeoutMut;
extern Mutex g_opMut;

bool init_balls(void);

bool ban_add(const char* nickname, const char* udid, const char* ip);
bool ban_revoke(const char* argument);
bool ban_check(const char* udid, const char* ip, const char* nickname, bool* result);

bool timeout_set(const char* nickname, const char* udid, const char* ip, uint64_t timestamp);
bool timeout_revoke(const char* udid, const char* ip);
bool timeout_check(const char* udid, const char* ip, uint64_t* result);

bool op_add(const char* nickname, const char* ip);
bool op_revoke(const char* ip);
bool op_check(const char* ip, uint8_t* level);

bool whitelist_add(const char* nickname, const char* ip);
bool whitelist_revoke(const char* argument);
bool whitelist_check(const char* ip, bool* result);

#endif
