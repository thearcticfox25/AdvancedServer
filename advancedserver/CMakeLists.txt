option(DYLIB "Builds dynamic library" OFF)

# Needed for packet stuff
include(TestBigEndian)
TEST_BIG_ENDIAN(IS_BIG_ENDIAN)

if(IS_BIG_ENDIAN)
        add_compile_definitions(SYS_BIG_ENDIAN)
endif()

if(DYLIB)
	add_compile_definitions(SYS_DYLIB)
endif()

if((CMAKE_C_COMPILER_ID STREQUAL "Clang" OR CMAKE_C_COMPILER_ID STREQUAL "GNU"))
	set(CMAKE_C_FLAGS_DEBUG "-g")
	set(CMAKE_C_FLAGS_RELEASE "-O3")
	message("Clang/GCC flags applied")
endif()

set(SOURCES
        "Config.c"
        "Moderation.c"
        "Status.c"
        "cJSON.c"
        "CMath.c"
        "DyList.c"
        "Log.c"
        "Lib.c"
        "Server.c"
        "Vote.c"
        "Maps.c"
        "Packet.c"
        "Player.c"
        "Palette.c"
        "Zone.c"

        "Lobby.c"
        "Mapvote.c"
        "CharSelect.c"
        "Game.c"
        "Results.c"

        "io/Time.c"
        "io/File.c"

        "entities/Ring.c"
        "entities/CreamRing.c"
        "entities/TailsProjectile.c"
        "entities/EggmanTracker.c"
        "entities/BlackRing.c"
        "entities/ExellerClone.c"
        "entities/RMZSlug.c"
        "entities/RMZShard.c"
        "entities/Act9Wall.c"
        "entities/NotPerfect.c"
        "entities/PFLift.c"
        "entities/HDDoor.c"
        "entities/SpkieController.c"
        "entities/LCEye.c"
        "entities/LCChain.c"
        "entities/NAPSnowball.c"
        "entities/NAPIce.c"
        "entities/HillThunder.c"
        "entities/VVLava.c"
        "entities/YouCantRun.c"
        "entities/TCAcid.c"
        "entities/KAFSpeedBox.c"
        "entities/TailsDoll.c"
        "entities/DTBall.c"
        "entities/DTStalactits.c"
        "entities/Dummy.c"
        "entities/WDLatern.c"
        "entities/MJLava.c"
        "entities/MJAss.c"
        "entities/MJJudger.c"

        "maps/RavineMist.c"
        "maps/Act9.c"
        "maps/NotPerfect.c"
        "maps/PricelessFreedom.c"
        "maps/HauntingDream.c"
        "maps/DotDotDot.c"
        "maps/LimpCity.c"
        "maps/NastyParadise.c"
        "maps/Hill.c"
        "maps/VolcanoValley.c"
        "maps/YouCantRun.c"
        "maps/TortureCave.c"
        "maps/KindAndFair.c"
        "maps/DarkTower.c"
        "maps/FartZone.c"
        "maps/MajinForest.c"
        "maps/HideAndSeek.c"
        "maps/WoodDream.c"
        "maps/Marijuna.c"
)

if(DYLIB) # Build .dll or .so
		add_library(AdvancedServer SHARED ${SOURCES})

		if (WIN32)
			target_link_libraries(AdvancedServer PRIVATE enet ws2_32 winmm dbghelp)
		else() # UNIX
			set(THREADS_PREFER_PTHREAD_FLAG ON)
			find_package(Threads REQUIRED)
			target_link_libraries(AdvancedServer PRIVATE enet m Threads::Threads)
		endif()
else() # Build executable
		add_executable(AdvancedServer 
			${SOURCES}
			Main.c
		)

		if (WIN32)
			target_link_libraries(AdvancedServer PRIVATE enet ws2_32 winmm dbghelp)
		else() # UNIX
			set(THREADS_PREFER_PTHREAD_FLAG ON)
			find_package(Threads REQUIRED)
			target_link_libraries(AdvancedServer PRIVATE enet m Threads::Threads)
		endif()
endif()


if(MSVC)
        add_definitions(-D_CRT_SECURE_NO_WARNINGS)
        set(CMAKE_C_FLAGS_RELEASE "${CMAKE_C_FLAGS_RELEASE} /MT")
endif()

unset(DYLIB CACHE)
