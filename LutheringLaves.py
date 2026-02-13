
import os
import shutil
import hashlib
import json
import gzip
import io
import argparse
import logging
import subprocess
from enum import Enum
from pathlib import Path
from urllib.request import urlopen, Request, HTTPError
from urllib.parse import urljoin,quote

logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s [%(levelname)s] %(message)s',
    handlers=[logging.FileHandler('launcher.log', encoding='utf-8'), logging.StreamHandler()]
)
logger = logging.getLogger("LutheringLaves")

# 适配 nuitka 打包后证书丢失与文件路径异常问题
import certifi
os.environ["SSL_CERT_FILE"] = certifi.where()
import sys
base_dir = os.path.dirname(sys.argv[0])
logger.info(f"base dir: {base_dir}")

WW_LAUNCHER_DOWNLOAD_API = 'https://prod-cn-alicdn-gamestarter.kurogame.com/launcher/launcher/10003_Y8xXrXk65DqFHEDgApn3cpK5lfczpFx5/G152/index.json'
WW_LAUNCHER_API = 'https://prod-cn-alicdn-gamestarter.kurogame.com/launcher/game/G152/10003_Y8xXrXk65DqFHEDgApn3cpK5lfczpFx5/index.json'

class LauncherState(Enum):
    STARTGAME = 0
    GAMERUNNING =1
    NEEDINSTALL = 2
    DOWNLOADING = 3
    VALIDATING = 4
    NEEDUPDATE = 5
    UPDATING = 6
    MERGEING = 7
    NETWORKERROR = 8

class ProgressInfo:
    def __init__(self):
        self.total_size = 0
        self.finished_size = 0
        self.total_count = 0
        self.finished_count = 0
        
class Launcher:
    
    _instance = None
    _initialized = False
    
    def __new__(cls, game_folder):
        if cls._instance is None:
            cls._instance = super(Launcher, cls).__new__(cls)
        return cls._instance
    
    def __init__(self, game_folder):
        if Launcher._initialized:
            return
        
        self.launcher_api = WW_LAUNCHER_API
        self.launcher_info = self.get_result(self.launcher_api)
        
        if self.launcher_info is None:
            self.state = LauncherState.NETWORKERROR
            return
        
        self.cdn_node = self.select_cdn()
        
        # create game folder
        self.game_folder_path = Path(base_dir) / Path(game_folder)
        logger.info(f"Game folder path: {self.game_folder_path}")
        if not self.game_folder_path.exists():
            logger.info(f"Creating game folder: {self.game_folder_path}")
            self.game_folder_path.mkdir()
            
        self.temp_folder_path = None
        
        # get last version game filelist
        self.gamefile_index = self.get_gamefile_index()
            
        # download url middle path
        self.resources_base_path = self.launcher_info['default']['resourcesBasePath'] if self.launcher_info else None
        self.current_version = self.launcher_info['default']['version'] if self.launcher_info else None
        self.local_version = self.get_localVersion()
        
        self.download_game_progress = ProgressInfo()
        self.verify_game_progress = ProgressInfo()
        self.update_game_progress = ProgressInfo()
        self.update_game_progress_patch = ProgressInfo()
        
        self.support_incremental_patching = False
        self.target_patch = None
        self.gamefile_index_patch = None
        self.resources_base_path_patch = None
        self.krdiff_file_path = None
        self.progress_callback = None
        
        self.init_launcher_state()
        self.init_incremental_update()
        self.init_launcher_settings()
        self.init_background()
    
    def init_launcher_state(self):
        self.state = LauncherState.STARTGAME
        logger.info("set launcher state to STARTGAME")
        
        if self.local_version is None:
            resource_list = list(self.gamefile_index['resource'])
            for file in resource_list:
                file_path = self.game_folder_path.joinpath(Path(file['dest']))
                if not file_path.exists():
                    self.state = LauncherState.NEEDINSTALL
                    logger.info("set launcher state to NEEDINSTALL")
                    return
        else:
            if self.current_version != self.local_version:
                self.state = LauncherState.NEEDUPDATE
                logger.info("set launcher state to NEEDUPDATE")
                return
    
    def init_launcher_settings(self):
        settings_file_path = Path(base_dir) / 'settings.json'
        if not settings_file_path.exists():
            default_settings = {
                "proton_version": "",
                "proton_path": "",
                "steamappid": "0",
                "proton_media_use_gst": "0",
                "proton_enable_wayland": "0",
                "proton_no_d3d12": "0",
                "mangohud": "0"
            }

            if self.get_latest_proton():
                default_settings['proton_version'] = self.get_latest_proton()['version']
                default_settings['proton_path'] = self.get_latest_proton()['proton_path']

            with open(settings_file_path, 'w', encoding='utf-8') as f:
                json.dump(default_settings, f, ensure_ascii=False, indent=4)
                
        with open(settings_file_path, 'r', encoding='utf-8') as f:
            settings = json.load(f)
            self.settings = settings
            
    def init_background(self):
        background_config = {
            'background': 'background.webp',
            'slogan': 'slogan.png'
        }
        self.background_config = background_config
        launcher_download_info = self.get_result(WW_LAUNCHER_DOWNLOAD_API)
        if launcher_download_info is None: return
        if launcher_download_info.get('functionCode', None) is None: return
        
        function_codes = launcher_download_info['functionCode']['background']
        background_info_api = f'https://prod-cn-alicdn-gamestarter.kurogame.com/launcher/10003_Y8xXrXk65DqFHEDgApn3cpK5lfczpFx5/G152/background/{function_codes}/zh-Hans.json'
        background_info = self.get_result(background_info_api)
        
        if background_info is None: return
        
        background_file_name = background_info['firstFrameImage'].split('/')[-1]
        slogan_file_name = background_info['slogan'].split('/')[-1]

        bd_download = self.download_file_with_resume(background_info['firstFrameImage'], Path(base_dir) / Path('resource') / Path(background_file_name))
        sg_download = self.download_file_with_resume(background_info['slogan'], Path(base_dir) / Path('resource') / Path(slogan_file_name))
        
        if bd_download and sg_download:
            self.background_config['background'] = background_file_name
            self.background_config['slogan'] = slogan_file_name
        
        
    def get_gamefile_index(self):
        if self.launcher_info is None: return None
        
        indexfile_uri = self.launcher_info['default']['config']['indexFile']
        indexfile = self.get_result(urljoin(self.cdn_node, indexfile_uri))
        
        if not indexfile: return None
        
        return indexfile
        
    def download_game(self):
        logger.info('Start downloading game client files...')
        self.state = LauncherState.DOWNLOADING
        resource_list = list(self.gamefile_index['resource'])
        self.download_game_progress.total_count = len(resource_list)
        for resource in resource_list:
            self.download_game_progress.total_size += resource['size']
        length = self.download_game_progress.total_count
        logger.info(f'Total resource files: {length}')
        for file in resource_list:
            download_url = urljoin(self.cdn_node, self.resources_base_path + "/" + file['dest'])
            download_url = quote(download_url, safe=':/')
            file_size = int(file['size'])
            file_path = self.game_folder_path.joinpath(Path(file['dest']))
            downloaded_count = self.download_game_progress.finished_count
            logger.info(f"Downloading file {downloaded_count + 1} / {length}: {file_path}")
            self.download_file_with_resume(url=download_url, file_path=file_path, flag='download', file_size=file_size)
            self.download_game_progress.finished_count += 1
    
    def update_game(self):
        logger.info('Starting update game client files...')
        self.state = LauncherState.UPDATING
        resource_list = list(self.gamefile_index['resource'])
        self.update_game_progress.total_count = len(resource_list)
        for resource in resource_list:
            self.update_game_progress.total_size += resource['size']
        length = self.update_game_progress.total_count
        for file in resource_list:
            file_path = self.game_folder_path.joinpath(Path(file['dest']))
            current_md5 = self.get_file_md5(file_path)
            updated_count = self.update_game_progress.finished_count
            logger.info(f"Updataing file {updated_count + 1} / {length}: {file_path}")
            if current_md5 == file['md5']:
                self.update_progress(flag='update',value=int(file['size']))
                self.update_game_progress.finished_count += 1
                logger.info(f'{file_path} MD5 match')
                continue
            logger.warning(f'{file_path} MD5 mismatch (expected: {file["md5"]}, got: {current_md5})')
            download_url = urljoin(self.cdn_node, self.resources_base_path + "/" + file['dest'])
            download_url = quote(download_url, safe=':/')
            self.download_file_with_resume(url=download_url, file_path=file_path, overwrite=True, flag='update')
            self.update_game_progress.finished_count += 1
    
    def download_patch(self):
        
        self.temp_folder_path = self.game_folder_path.parent / 'temp_folder'
        if not self.temp_folder_path.exists():
            self.temp_folder_path.mkdir()
        
        krdiff_file_path = None
        
        for i, file in enumerate(self.gamefile_index_patch['resource']):
            length = len(self.game_folder_path['resource'])
            if 'fromFolder' in file:
                download_url = urljoin(self.cdn_node, file['fromFolder'] + "/" + file['dest'])
                download_url = quote(download_url, safe=':/')
                file_path = self.temp_folder_path.joinpath(Path(file['dest']))
                logger.info(f"Downloading file {i+1}/{length}: {file_path}")
                self.download_file_with_resume(url=download_url, file_path=file_path)
                continue
            
            download_url = urljoin(self.cdn_node,  self.resources_base_path_patch + "/" + file['dest'])
            download_url = quote(download_url, safe=':/')
            krdiff_file_path = Path(base_dir) / Path(file['dest'])
            logger.info(f"Downloading file {i+1}/{length}: {krdiff_file_path}")
            self.download_file_with_resume(url=download_url, file_path=krdiff_file_path)
    
    def merge_patch(self):
        
        if self.krdiff_file_path:
            self.run_hpatchz(self.krdiff_file_path, self.game_folder_path, self.temp_folder_path)
            
            for item in self.temp_folder_path.rglob('*'):
                relative_path = item.relative_to(self.temp_folder_path)
                destination = self.game_folder_path / relative_path
                
                if destination.exists():
                    if destination.is_file():
                        destination.unlink()
                    else:
                        shutil.rmtree(str(destination))
                
                shutil.move(str(item), str(destination))
                
            shutil.rmtree(str(self.temp_folder_path))
            self.krdiff_file_path.unlink()
    
    def verify_gamefile(self):
        self.state = LauncherState.VALIDATING
        resource_list = list(self.gamefile_index['resource'])
        
        chunk_paks = []
        
        for resource in resource_list:
            self.verify_game_progress.total_size += resource['size']
            if resource['dest'].startswith('Client/Content/Paks/'):
                chunk_paks.append(resource['dest'].split('/')[-1])
                
        print(chunk_paks)
                
        # 删除无效的pak文件
        local_chunk_paks = os.listdir(self.game_folder_path / 'Client' / 'Content' / 'Paks')
        for chunk_pak in local_chunk_paks:
            if chunk_pak not in chunk_paks:
                remove_file = self.game_folder_path / 'Client' / 'Content' / 'Paks' / chunk_pak
                logger.warning(f'Chunk pak {chunk_pak} will be removed')
                if remove_file.exists():
                    remove_file.unlink()
    
        for file in resource_list:
            file_path = self.game_folder_path.joinpath(Path(file['dest']))
            
            current_md5 = self.get_file_md5(file_path)

            if current_md5 == file['md5']:
                logger.info(f'{file_path} MD5 match')
                self.update_progress(flag='verify',value=int(file['size']))
                continue
            
            logger.warning(f'{file_path} MD5 mismatch (expected: {file["md5"]}, got: {current_md5})')
            download_url = urljoin(self.cdn_node, self.resources_base_path + "/" + file['dest'])
            download_url = quote(download_url, safe=':/')
            self.download_file_with_resume(url=download_url, file_path=file_path, overwrite=True)
            
            current_md5 = self.get_file_md5(file_path)
            if current_md5 == file['md5']:
                logger.info(f'{file_path} MD5 OK after re-download')
            else:
                logger.error(f'{file_path} Still MD5 mismatch after re-download')
            self.update_progress(flag='verify',value=int(file['size']))
            
        self.update_localVersion()
    
    def get_result(self, url):
        try:
            req = Request(url, headers={
                'User-Agent': 'Mozilla/5.0',
                'Accept-Encoding': 'gzip'
            })
            with urlopen(req, timeout=10) as rsp:
                if rsp.status != 200:
                    logger.error(f"HTTP status {rsp.status} for {url}")
                    return None
                content_encoding = rsp.headers.get('Content-Encoding', '').lower()
                data = rsp.read()
                if 'gzip' in content_encoding:
                    try:
                        with gzip.GzipFile(fileobj=io.BytesIO(data)) as f:
                            data = f.read()
                    except Exception as e:
                        logger.error(f"Gzip decompression error: {str(e)}")
                        return None
                try:
                    return json.loads(data.decode('utf-8'))
                except UnicodeDecodeError:
                    try:
                        return json.loads(data.decode('gbk'))
                    except:
                        logger.error("Failed to decode JSON response")
                        return None        
        except HTTPError as e:
            logger.error(f"HTTP Error {e.code}: {e.reason}")
            return None
        except Exception as e:
            logger.error(f"Error fetching patch info: {str(e)}")
            return None
    
    def select_cdn(self):
        if self.launcher_info is None: return None
        
        cdnlist = self.launcher_info['default'].get('cdnList', None)
        
        if not cdnlist: return None
        
        available_nodes = [node for node in cdnlist if node['K1'] == 1 and node['K2'] == 1]
        if not available_nodes:
            return None
        
        max_priority = max(node['P'] for node in available_nodes)
        
        for node in available_nodes:
            if node['P'] == max_priority:
                return node['url']
    
    def get_localVersion(self):
        file_path = self.game_folder_path / "launcherDownloadConfig.json"
        if not os.path.exists(file_path):
            return None
        with open(file_path, 'r', encoding='utf-8') as file:
            data = json.load(file)
            return data.get('version', None)
    
    def update_localVersion(self):
        new_version = self.current_version
        temp = {
            "version":new_version,
            "reUseVersion":"",
            "state":"",
            "isPreDownload":False,
            "appId":"10003"
        }
        file_path = self.game_folder_path / "launcherDownloadConfig.json"
        with open(file_path, 'w', encoding='utf-8') as file:
            json.dump(temp, file, ensure_ascii=False)
    
    def download_file_with_resume(self, url, file_path, overwrite=False, flag=None, file_size=None):
        directory = file_path.parent
        if not directory.exists():
            os.makedirs(directory)
        
        if os.path.exists(file_path):
            if not overwrite:
                if file_size:
                    self.update_progress(flag=flag, value=file_size)
                logger.info(f'{file_path} already exists. Skipping download.')
                return True
            else:
                os.remove(file_path)
                logger.info(f'{file_path} is deleted and start re-download.')
        
        temp_file_path = directory / f'{file_path.name}.temp'
        downloaded_bytes = 0
        if os.path.exists(temp_file_path):
            downloaded_bytes = os.path.getsize(temp_file_path)
            
        headers = {'User-Agent': 'Mozilla/5.0'}
        if downloaded_bytes > 0:
            headers['Range'] = f'bytes={downloaded_bytes}-'
            
        content_length = 0
        
        try:
            req = Request(url, headers=headers)
            with urlopen(req, timeout=10) as rsp:
                if rsp.status == 206:
                    content_length = int(rsp.headers.get('Content-Length'))
                    total_size = downloaded_bytes + content_length
                elif rsp.status == 200:
                    content_length = int(rsp.headers.get('Content-Length'))
                    total_size = content_length if content_length else 0
                    if downloaded_bytes > 0:
                        logger.warning("Server doesn't support resume, restarting download")
                        downloaded_bytes = 0
                else:
                    logger.error(f"Unexpected HTTP status: {rsp.status}")
                    return False
                
                mode = "ab" if downloaded_bytes > 0 else "wb"
                with open(temp_file_path, mode) as file:
                    while True:
                        chunk = rsp.read(1024 * 1024)
                        if not chunk:
                            break
                        file.write(chunk)
                        downloaded_bytes += len(chunk)
                        self.update_progress(flag=flag, value=len(chunk))
                        if total_size > 0:
                            percent = (downloaded_bytes / total_size) * 100
                            logger.info(f"{file_path} size:{total_size/1024/1024:.1f} MB {percent:.1f}%")
            
            shutil.move(temp_file_path, file_path)
            return True
        except Exception as e:
            logger.error(f"Download error: {str(e)}")
            if e.code == 416 and downloaded_bytes == content_length:
                if temp_file_path.exists(): 
                    shutil.move(temp_file_path, file_path)
            return False
            
    
    def download_patch_tool(self):
        
        if os.name == "nt":
            tool_url = "https://gitee.com/tiz/LutheringLaves/raw/main/tools/hpatchz.exe"
            file_name = Path(base_dir) / Path("tools") / "hpatchz.exe"
        if os.name == "posix":
            tool_url = "https://gitee.com/tiz/LutheringLaves/raw/main/tools/hpatchz"
            file_name = Path(base_dir) / Path("tools") / "hpatchz"
            
        if not self.download_file_with_resume(tool_url, file_name): return False
        
        if os.name == "posix":
            os.system(f"chmod +x {str(file_name)}")
        
        return True
    
    def run_hpatchz(self,patch_path, original_path, output_path):
        self.download_patch_tool()
        if os.name == "nt":
            cmd = f'tools\hpatchz.exe "{original_path}" {patch_path} "{output_path}" -f'
        if os.name == "posix":
            cmd = f'tools/hpatchz "{original_path}" {patch_path} "{output_path}" -f'
        os.system(cmd)

    def is_support_incremental_patching(self):
        if not self.local_version: return False
        
        patch_configs = self.launcher_info['default']['config']['patchConfig']
        
        target_patch = list(filter(lambda x: x['version'] == self.local_version, patch_configs))
        
        if len(target_patch) == 0: return False
        
        if len(target_patch[0]['ext']) == 0: return False
    
    def init_incremental_update(self):
        
        if not self.local_version: return
        
        patch_configs = self.launcher_info['default']['config']['patchConfig']
        
        target_patch = list(filter(lambda x: x['version'] == self.local_version, patch_configs))
        
        if len(target_patch) == 0: return
        
        if len(target_patch[0]['ext']) == 0: return
        
        self.support_incremental_patching = True
        self.target_patch = target_patch
        self.resources_base_path_patch = self.target_patch[0]['baseUrl']
        indexfile_uri = target_patch[0]['indexFile']
        self.gamefile_index_patch = self.get_result(urljoin(self.cdn_node, indexfile_uri))
    
    def set_progress_callback(self, callback):
        self.progress_callback = callback
    
    def update_progress(self, flag, value):
        if flag == "download":
            self.download_game_progress.finished_size += value
        elif flag == "update":
            self.update_game_progress.finished_size += value
        elif flag == "verify":
            self.verify_game_progress.finished_size += value
        elif flag == "update_patch":
            self.update_game_progress_patch.finished_size += value
        
        mutil_progress = {
            "download": self.download_game_progress,
            "verify": self.verify_game_progress,
            "update": self.update_game_progress,
            "update_patch": self.update_game_progress_patch,
        }
        
        if self.progress_callback:
            self.progress_callback(mutil_progress, flag) 

    def get_file_md5(self, file_path):
        md5_hash = hashlib.md5()
        try:
            with open(file_path, "rb") as file:
                for chunk in iter(lambda: file.read(4096), b""):
                    md5_hash.update(chunk)
            return md5_hash.hexdigest()
        except FileNotFoundError:
            logger.error(f"The file {file_path} does not exist.")
            return None
    
    def start_game_process(self):
        logger.info("Launching game...")
        if os.name == "nt":
            game_exe = self.launcher.game_folder_path / "Wuthering Waves.exe"
            self.game_process = subprocess.Popen(f'"{game_exe}"', shell=True)
        if os.name == "posix":
            base_dir = os.path.dirname(sys.argv[0])
            
            game_exe_path =  Path(base_dir) / "Wuthering Waves Game" / "Wuthering Waves.exe"
            steam_dir_path = Path(os.path.expanduser("~")) / '.steam' / 'steam'
            proton_path = self.settings.get('proton_path', '')
            
            steamAppid = self.settings.get('steamappid', '0')
            
            compatdata_path = Path(base_dir) / "compatdata" / steamAppid
            wine_prefix = compatdata_path / "pfx" 
            
            if not compatdata_path.exists():
                compatdata_path.mkdir(parents=True, exist_ok=True)
            compatdata_path = compatdata_path.resolve()
            
            os.environ["STEAM_COMPAT_DATA_PATH"] = str(compatdata_path)
            os.environ["STEAM_COMPAT_CLIENT_INSTALL_PATH"] = str(steam_dir_path)
            os.environ["STEAM_PROTON_PATH"] = str(proton_path)
            os.environ["WINEPREFIX"] = str(wine_prefix)
            
            if steamAppid != '0':
                os.environ["STEAMAPPID"] = steamAppid
            if self.settings.get('proton_media_use_gst', '0') == '1':
                os.environ["PROTON_MEDIA_USE_GST"] = "1"
            if self.settings.get('proton_enable_wayland', '0') == '1':
                os.environ["PROTON_ENABLE_WAYLAND"] = "1"
            if self.settings.get('proton_no_d3d12', '0') == '1':
                os.environ["PROTON_NO_D3D12"] = "1"
            if self.settings.get('mangohud', '0') == '1':
                os.environ["MANGOHUD"] = "1"
            
            os.environ["STEAMDECK"] = "1"
            
            logger.info(f"compatdata_path: {compatdata_path}")
            # steam_launch_command = f"SteamDeck=1 /home/deck/.local/share/Steam/ubuntu12_32/steam-launch-wrapper \
            #     -- /home/deck/.local/share/Steam/ubuntu12_32/reaper \
            #     SteamLaunch AppId={AppId} \
            #     -- /home/deck/.local/share/Steam/steamapps/common/SteamLinuxRuntime_sniper/_v2-entry-point \
            #     --verb=waitforexitandrun \
            #     -- {proton_path} \
            #     waitforexitandrun \
            #     {game_exe}"
            try:
                self.game_process = subprocess.Popen([
                    proton_path,
                    "waitforexitandrun",
                    game_exe_path
                ])
                logger.info(f"Launched game with Proton: {proton_path} run {game_exe_path}")
            except Exception as e:
                logger.error(f"Failed to launch game with Proton: {e}")
                
    def stop_game_process(self):
        if hasattr(self, 'game_process') and self.game_process:
            logger.info("Stopping game process...")
            os.system("kill -9 $(pgrep -f 'Wuthering Waves')")
            self.game_process.terminate()
            self.game_process.wait()
            logger.info("Game process stopped.")
            self.state = LauncherState.STARTGAME
            
    def find_available_proton(self):
        # Find all available Proton versions
        geproton_versions = []
        geproton_dir_path = Path(os.path.expanduser("~")) / '.steam' / 'steam' / 'compatibilitytools.d'
        for entry in os.listdir(geproton_dir_path):
            if entry.startswith("GE-Proton"):
                proton_file_path = geproton_dir_path / entry / "proton"
                version_file_path = geproton_dir_path / entry / "version"
                if proton_file_path.exists() and version_file_path.exists():
                    with open(version_file_path, 'r') as f:
                        version_content = f.read().strip()
                        timestamp_str, version = version_content.split(' ')
                    proton_version_dict = {'proton_path': str(proton_file_path), 'timestamp':timestamp_str, 'version':version}
                    geproton_versions.append(proton_version_dict)
        geproton_versions.sort(key=lambda x: x['timestamp'], reverse=True)
        
        proton_versions = []
        proton_dir_path = Path(os.path.expanduser("~")) / '.steam' / 'steam' / 'steamapps' / 'common'
        for entry in os.listdir(proton_dir_path):
            if entry.startswith("Proton"):
                proton_file_path = proton_dir_path / entry / "proton"
                version_file_path = proton_dir_path / entry / "version"
                if proton_file_path.exists() and version_file_path.exists():
                    with open(version_file_path, 'r') as f:
                        version_content = f.read().strip()
                        timestamp_str, version = version_content.split(' ')
                    proton_version_dict = {'proton_path':str(proton_file_path), 'timestamp':timestamp_str, 'version':version}
                    proton_versions.append(proton_version_dict)
        proton_versions.sort(key=lambda x: x['timestamp'], reverse=True)
        
        return geproton_versions, proton_versions
    
    def has_available_proton(self):
        geproton_versions, proton_versions = self.find_available_proton()
        if len(geproton_versions) == 0 or len(proton_versions) == 0:
            return False
        
    def get_latest_proton(self):
        geproton_versions, proton_versions = self.find_available_proton()
        if len(geproton_versions) > 0:
            return geproton_versions[0]
        if len(proton_versions) > 0:
            return proton_versions[0]
        return None
    
    def update_settings(self):
        settings_file_path = Path(base_dir) / 'settings.json'
        with open(settings_file_path, 'w', encoding='utf-8') as f:
            json.dump(self.settings, f, ensure_ascii=False, indent=4)

if __name__ == '__main__':
    
    parser = argparse.ArgumentParser()
    parser.add_argument('--mode', default='install',help='install or update or patch-update')
    parser.add_argument('--folder', default='Wuthering Waves Game',help='set download folder')
    args = parser.parse_args()
    
    launcher = Launcher(game_folder=args.folder)
    launcher.verify_gamefile()
    # download game client file
    if args.mode == 'install':
        launcher.download_game()
        launcher.verify_gamefile()
        launcher.update_localVersion()  
    
    # update game client file
    if args.mode == 'update':
        launcher.update_game()
        launcher.verify_gamefile()
        launcher.update_localVersion()  
    
    # Incremental updates
    if args.mode == 'patch-update':
        launcher.download_patch()
        launcher.merge_patch()