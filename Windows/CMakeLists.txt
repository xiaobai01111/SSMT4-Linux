cmake_minimum_required(VERSION 3.16)
project(ssmt4-bridge LANGUAGES CXX C)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Third-party C libraries
add_library(third_party STATIC
    third_party/cJSON/cJSON.c
    third_party/sqlite3/sqlite3.c
)
target_include_directories(third_party PUBLIC
    third_party/cJSON
    third_party/sqlite3
)
# sqlite3 needs dl and pthread on some platforms, but on Windows we don't need them
if(WIN32)
    target_compile_definitions(third_party PRIVATE SQLITE_OS_WIN=1)
endif()

# Main executable
add_executable(ssmt4-bridge
    src/main.cpp
    src/bridge_config.cpp
    src/status_reporter.cpp
    src/ini_handler.cpp
    src/d3dx_ini_configurator.cpp
    src/mod_manager.cpp
    src/ini_validator.cpp
    src/package_deployer.cpp
    src/signature_verifier.cpp
    src/dll_injector.cpp
    src/process_tracker.cpp
    src/game_initializer.cpp
    src/wwmi_initializer.cpp
    src/local_storage.cpp
    src/xcmd_handler.cpp
    src/string_utils.cpp
    src/path_utils.cpp
)

target_include_directories(ssmt4-bridge PRIVATE include)
target_link_libraries(ssmt4-bridge PRIVATE third_party)

if(MINGW)
    # Use GUI (WINDOWS) subsystem, matching the original XXMI Launcher.
    # Console subsystem causes 3DMigoto's d3d11.dll DllMain to fail (error 1114)
    # because Wine/Proton handles DLL loading differently for console processes.
    # stdout/fprintf still works for the JSON protocol even with -mwindows.
    target_link_options(ssmt4-bridge PRIVATE -static-libgcc -static-libstdc++ -static -mwindows)
endif()

# Windows system libraries
if(WIN32)
    target_link_libraries(ssmt4-bridge PRIVATE
        kernel32 user32 shell32 bcrypt crypt32 psapi advapi32 shlwapi
    )
endif()
