#ifndef LIB_H
#define LIB_H
#include <enet/enet.h>
#include <io/Threads.h>
#include <DyList.h>
#include <Packet.h>

typedef enum
{
	DR_FAILEDTOCONNECT,
	DR_KICKEDBYHOST,
	DR_BANNEDBYHOST,
	DR_VERMISMATCH,
	DR_SERVERTIMEOUT,
	DR_PACKETSNOTRECV,
	DR_GAMESTARTED,
	DR_AFKTIMEOUT,
	DR_LOBBYFULL,
	DR_RATELIMITED,
	DR_SHUTDOWN,
	DR_IPINUSE,

	DR_DONTREPORT = 254,
	DR_OTHER = 255
} DisconnectReason;

struct Server;
struct PeerData;
typedef struct
{
	struct PeerData*	peer;
	String				ip_addr;
	bool				is_exe;
	int8_t				character;
} PeerInfo;

bool			disaster_init 	    (void);
void            disaster_shutdown   (void);
int				disaster_run   	    (void);
void 			disaster_terminate  (void);
bool            disaster_reboot     (void);

struct Server*	disaster_get(int);
int				disaster_count(void);

#endif
