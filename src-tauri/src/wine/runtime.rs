use std::path::Path;
use tracing::{info, warn};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RuntimeComponent {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
}

pub fn list_available_runtimes() -> Vec<RuntimeComponent> {
    vec![
        RuntimeComponent {
            id: "vcrun2005".into(),
            name: "Visual C++ 2005".into(),
            category: "vcrun".into(),
            description: "Microsoft Visual C++ 2005 Redistributable".into(),
        },
        RuntimeComponent {
            id: "vcrun2008".into(),
            name: "Visual C++ 2008".into(),
            category: "vcrun".into(),
            description: "Microsoft Visual C++ 2008 Redistributable".into(),
        },
        RuntimeComponent {
            id: "vcrun2010".into(),
            name: "Visual C++ 2010".into(),
            category: "vcrun".into(),
            description: "Microsoft Visual C++ 2010 Redistributable".into(),
        },
        RuntimeComponent {
            id: "vcrun2012".into(),
            name: "Visual C++ 2012".into(),
            category: "vcrun".into(),
            description: "Microsoft Visual C++ 2012 Redistributable".into(),
        },
        RuntimeComponent {
            id: "vcrun2013".into(),
            name: "Visual C++ 2013".into(),
            category: "vcrun".into(),
            description: "Microsoft Visual C++ 2013 Redistributable".into(),
        },
        RuntimeComponent {
            id: "vcrun2015".into(),
            name: "Visual C++ 2015-2022".into(),
            category: "vcrun".into(),
            description: "Microsoft Visual C++ 2015-2022 Redistributable".into(),
        },
        RuntimeComponent {
            id: "vcrun2022".into(),
            name: "Visual C++ 2022".into(),
            category: "vcrun".into(),
            description: "Microsoft Visual C++ 2022 Redistributable".into(),
        },
        RuntimeComponent {
            id: "dotnet35".into(),
            name: ".NET Framework 3.5".into(),
            category: "dotnet".into(),
            description: "Microsoft .NET Framework 3.5".into(),
        },
        RuntimeComponent {
            id: "dotnet40".into(),
            name: ".NET Framework 4.0".into(),
            category: "dotnet".into(),
            description: "Microsoft .NET Framework 4.0".into(),
        },
        RuntimeComponent {
            id: "dotnet48".into(),
            name: ".NET Framework 4.8".into(),
            category: "dotnet".into(),
            description: "Microsoft .NET Framework 4.8".into(),
        },
        RuntimeComponent {
            id: "d3dx9".into(),
            name: "DirectX 9".into(),
            category: "directx".into(),
            description: "DirectX 9 runtime libraries".into(),
        },
        RuntimeComponent {
            id: "d3dx10".into(),
            name: "DirectX 10".into(),
            category: "directx".into(),
            description: "DirectX 10 runtime libraries".into(),
        },
        RuntimeComponent {
            id: "d3dcompiler_43".into(),
            name: "D3DCompiler 43".into(),
            category: "directx".into(),
            description: "Direct3D Compiler 43".into(),
        },
        RuntimeComponent {
            id: "d3dcompiler_47".into(),
            name: "D3DCompiler 47".into(),
            category: "directx".into(),
            description: "Direct3D Compiler 47".into(),
        },
        RuntimeComponent {
            id: "cjkfonts".into(),
            name: "CJK Fonts".into(),
            category: "fonts".into(),
            description: "Chinese/Japanese/Korean fonts".into(),
        },
        RuntimeComponent {
            id: "fakechinese".into(),
            name: "Fake Chinese".into(),
            category: "fonts".into(),
            description: "Fake Chinese font replacement".into(),
        },
        RuntimeComponent {
            id: "mf".into(),
            name: "Media Foundation".into(),
            category: "media".into(),
            description: "Windows Media Foundation".into(),
        },
        RuntimeComponent {
            id: "xact".into(),
            name: "XAudio/XACT".into(),
            category: "media".into(),
            description: "XAudio and XACT audio libraries".into(),
        },
    ]
}

pub fn check_winetricks_available() -> bool {
    which::which("winetricks").is_ok()
}

pub async fn install_runtime(
    prefix_path: &Path,
    wine_path: &Path,
    component: &str,
) -> Result<String, String> {
    if !check_winetricks_available() {
        return Err("winetricks is not installed. Please install winetricks first (e.g. 'sudo apt install winetricks' or 'sudo pacman -S winetricks').".to_string());
    }

    info!(
        "Installing runtime component '{}' to prefix {}",
        component,
        prefix_path.display()
    );

    let output = tokio::process::Command::new("winetricks")
        .env("WINEPREFIX", prefix_path)
        .env("WINE", wine_path)
        .arg("-q")
        .arg(component)
        .output()
        .await
        .map_err(|e| format!("Failed to run winetricks: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        info!("Successfully installed {}", component);
        Ok(format!("Installed {}", component))
    } else {
        warn!("winetricks failed for {}: {}", component, stderr);
        Err(format!(
            "Failed to install {}: {}{}",
            component, stdout, stderr
        ))
    }
}
