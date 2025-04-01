#include <enet/enet.h>
#include <io/Dir.h>
#include <io/Threads.h>
#include <Lib.h>
#include <Log.h>
#include <Server.h>
#include <States.h>
#include <DyList.h>
#include <Config.h>
#include <Moderation.h>
#include <Status.h>

ThreadVar		g_threadName;
DyList			servers;
bool			running = 0;

bool allocate_server(uint16_t base_port, uint16_t n)
{
	const Server templ = {
		.state = ST_LOBBY,
		.last_map = -1,
		.id = n,
		.running = true,
		.delta = 1,
		.game = {
			.exe = -1
		},
		.lobby = {
			.vote = {
				.ongoing = 0,
			},
			.prac_countdown = 0
		}
	};

	Server* server = malloc(sizeof(Server));
	RAssert(server);
	memcpy(server, &templ, sizeof(Server));

    for (int i = 0; i < MAP_COUNT; i++)
		server->map_pickrates[i] = 255;

	// Init lobby
	MutexCreate(server->state_lock);
    RAssert(dylist_create(&server->peers, g_config.pairing.maximum_players_per_lobby));
	
	ENetAddress addr;
	addr.host = ENET_HOST_ANY;
	addr.port = base_port + n;
	server->host = enet_host_create(&addr, 50, 2, 0, 0);

	Info("Listening on port %d.", base_port + n);
	RAssert(server->host != NULL);
	RAssert(lobby_init(server));
	RAssert(dylist_push(&servers, server));

	return true;
}

bool deallocate_server(Server* server)
{
    if (!server)
        return false;

    if (server->host) {
        enet_host_destroy(server->host);
        server->host = NULL;
    }
    MutexDestroy(server->state_lock);
    dylist_free(&server->peers);
    free(server);
    return true;
}

bool disaster_init(void)
{
	if (running)
		return true;

	RAssert(enet_initialize() == 0);

    char os[16];

    #ifdef _WIN32
        snprintf(os, 16, "Windows");
    #elif defined(__APPLE__) && defined(__MACH__)
        snprintf(os, 16, "OS X");
    #elif defined(__linux__)
        snprintf(os, 16, "Linux");
    #elif defined(__unix__)
        snprintf(os, 16, "Unix");
    #elif defined(__FreeBSD__)
        snprintf(os, 16, "FreeBSD");
    #else
        snprintf(os, 16, "Unknown OS");
    #endif

    char compiler[64];
    #ifdef __clang__
        snprintf(compiler, 64, "clang %d.%d.%d", __clang_major__, __clang_minor__, __clang_patchlevel__);
    #elif defined(__GNUC__)
        snprintf(compiler, 64, "gcc %d.%d.%d", __GNUC__, __GNUC_MINOR__, __GNUC_PATCHLEVEL__);
    #elif defined(_MSC_VER)
        snprintf(compiler, 64, "msvc %d", _MSC_VER);
    #else
        snprintf(compiler, 64, "unknown compiler");
    #endif

	// Init global variables
	ThreadVarCreate(g_threadName);
	ThreadVarSet(g_threadName, "Main Thr");

    Info("---------" LOG_RED "Advanced" LOG_BLU "Server" LOG_RST "---------");
    Info("Original Binary by: " LOG_YLW "Hander" LOG_RST);
    Info("Modded by: " LOG_PUR  "The Arctic Fox" LOG_RST);
    Info("Version: " LOG_BLU SERVER_VERSION LOG_RST);
    Info("Built on " LOG_PUR __DATE__ LOG_RST " at " LOG_GRN __TIME__ LOG_RST " for " LOG_RED "%s" LOG_RST " via " LOG_YLW "%s", os, compiler);
    Info("(c) 2024 Team Exe Empire");
	Info("--------------------------------");

	RAssert(config_init());
    RAssert(init_balls());
    RAssert(status_init());
	RAssert(log_init());

	RAssert(dylist_create(&servers, g_config.networking.server_count));
	for (int32_t i = 0; i < g_config.networking.server_count; i++)
		RAssert(allocate_server((uint16_t)g_config.networking.port, i));
	
	return true;
}

void disaster_shutdown(void)
{
    if (!running)
        return;

    // Cleanup each server
    for(int32_t i = 0; i < g_config.networking.server_count; i++) {
        Server* server = servers.ptr[i];
        deallocate_server(server);
    }

    dylist_free(&servers);
    log_uninit();
    enet_deinitialize();
    running = false;

    Info("Server shutdown complete.");
}

bool disaster_reboot(void)
{
    status_save();
    disaster_shutdown();
    disaster_init();
    return disaster_run();
}

int disaster_run(void)
{
	if (running)
        return 1;

	running = true;
	Debug("Entering main loop...");

	for(int32_t i = 0; i < g_config.networking.server_count; i++)
	{
		Server* server = servers.ptr[i];
		if(!server)
			continue;

		Thread th;
		ThreadSpawn(th, server_worker, server);
	}

	// dont ask too many questions
	while (running)
	{
		ThreadSleep(100);
	}
	
	return 0;
}

void disaster_terminate(void)
{
	if(!running)
		return;

    status_save();
	running = false;
	exit(0);
}

Server* disaster_get(int i)
{
	if (i < 0 || (size_t)i >= servers.capacity)
		return NULL;

	return (Server*)servers.ptr[i];
}

int disaster_count(void)
{
	return (int)servers.capacity;
}