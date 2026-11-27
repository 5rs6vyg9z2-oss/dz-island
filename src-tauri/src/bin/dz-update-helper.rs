#![cfg_attr(windows, windows_subsystem = "windows")]
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fs, io::{Read, Write}, path::{Path, PathBuf}, time::Duration};
use std::os::windows::process::CommandExt;
use zip::HasZipMetadata;
const APP: &str = "DZ-Island.exe";
const LEGACY_APP: &str = "Nectar.exe";
const ALLOWED: &[&str] = &[APP,LEGACY_APP,"DZUpdater.exe","WebView2Loader.dll","LICENSE.txt","LEIA-ME.txt","Perguntar-Na-Ilha.ps1","PERGUNTAS-NA-ILHA.txt","Battery/BatteryEngine.exe","Battery/headsetcontrol.exe","Battery/LICENSE.txt","Battery/THIRD_PARTY_NOTICES.txt"];
#[derive(Deserialize)]
struct Plan { root: PathBuf, parent: u32, version: String, digest: String }
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn child(root: &Path, name: &str) -> Result<PathBuf> {
    if !ALLOWED.contains(&name) { return Err("Unexpected update path".into()); }
    Ok(root.join(name))
}
fn copy_retry(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to.parent().unwrap())?;
    for attempt in 0..10 {
        match fs::copy(from,to) { Ok(_) => return Ok(()), Err(e) if attempt == 9 => return Err(e), Err(_) => std::thread::sleep(Duration::from_millis(200)) }
    }
    unreachable!()
}
fn move_retry(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to.parent().unwrap())?;
    for attempt in 0..10 {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(error) if attempt == 9 => return Err(error),
            Err(_) => std::thread::sleep(Duration::from_millis(200)),
        }
    }
    unreachable!()
}
fn validate(stage: &Path, plan: &Plan, app: &str) -> Result<Vec<String>> {
    let package=stage.join("package.zip");
    if fs::metadata(&package)?.len() > 150 * 1024 * 1024 { return Err("Package too large".into()); }
    let mut hash=Sha256::new(); let mut file=fs::File::open(&package)?; let mut buf=[0u8;65536];
    loop { let n=file.read(&mut buf)?; if n==0 {break} hash.update(&buf[..n]); }
    if format!("{:x}",hash.finalize()) != plan.digest { return Err("Package digest mismatch".into()); }
    let mut archive=zip::ZipArchive::new(fs::File::open(package)?)?;
    if archive.len()>30 { return Err("Too many package entries".into()); }
    let mut names=Vec::new(); let mut seen=HashSet::new(); let mut total=0u64;
    for i in 0..archive.len() {
        let mut entry=archive.by_index(i)?;
        let raw=entry.name()?.replace('\\',"/");
        if raw=="Battery/" {continue}
        let name=*ALLOWED.iter().find(|n| n.eq_ignore_ascii_case(&raw)).ok_or("Unexpected package entry")?;
        if matches!(name, APP | LEGACY_APP) && name != app { return Err("Unexpected application name".into()); }
        if !seen.insert(name) { return Err("Duplicate package entry".into()); }
        total=total.checked_add(entry.size()).ok_or("Package size overflow")?;
        if entry.size()>150*1024*1024 || total>250*1024*1024 { return Err("Unpacked package too large".into()); }
        let dest=child(&stage.join("unpack"),name)?;
        fs::create_dir_all(dest.parent().unwrap())?;
        let mut output=fs::OpenOptions::new().write(true).create_new(true).open(dest)?;
        let copied=std::io::copy(&mut entry.by_ref().take(150*1024*1024+1),&mut output)?;
        if copied!=entry.size() { return Err("Unpacked size mismatch".into()); }
        output.flush()?; names.push(name.to_owned());
    }
    if !seen.contains(app) { return Err("Missing application".into()); }
    let mut magic=[0;2];fs::File::open(stage.join("unpack").join(app))?.read_exact(&mut magic)?;
    if magic!=*b"MZ" { return Err("Invalid Windows application".into()); }
    Ok(names)
}
fn wait_parent(id:u32)->Result<()> {
    use windows::Win32::{Foundation::{CloseHandle,WAIT_TIMEOUT},System::Threading::{OpenProcess,WaitForSingleObject,PROCESS_SYNCHRONIZE}};
    if let Ok(handle)=unsafe{OpenProcess(PROCESS_SYNCHRONIZE,false,id)} {
        let status=unsafe{WaitForSingleObject(handle,60000)}; let _=unsafe{CloseHandle(handle)};
        if status==WAIT_TIMEOUT {return Err("Application did not exit".into())}
    }
    Ok(())
}
fn result(plan:&Plan,status:&str,message:&str) {
    let path=plan.root.join("data/update-result.json");
    let _=fs::create_dir_all(path.parent().unwrap());
    let _=fs::write(path,serde_json::json!({"status":status,"version":plan.version,"message":message}).to_string());
}
fn restart(root:&Path, app:&str)->Result<()> {
    std::process::Command::new(root.join(app)).current_dir(root).creation_flags(0x08000000).spawn()?;Ok(())
}
fn apply(stage:&Path,plan:&Plan,names:&[String],app:&str)->Result<()> {
    let mut changed=Vec::new();
    let installation=(||->Result<()> {
        for name in names {
            let target=child(&plan.root,name)?;let backup=child(&stage.join("backup"),name)?;
            let existed=target.is_file();
            // Windows can move a loaded executable even when it cannot overwrite it.
            // The staging directory and the application files share the same volume.
            if existed {
                move_retry(&target, &backup).map_err(|error| format!("Backing up {name}: {error}"))?;
            }
            changed.push((name.clone(),existed));
            copy_retry(&child(&stage.join("unpack"),name)?,&target)
                .map_err(|error| format!("Installing {name}: {error}"))?;
        } Ok(())
    })();
    if let Err(error)=installation {
        let mut message=error.to_string();
        for (name,existed) in changed.iter().rev() {
            let target=child(&plan.root,name)?;
            let restored = if *existed {
                move_retry(&child(&stage.join("backup"), name)?, &target)
            } else {
                match fs::remove_file(&target) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    outcome => outcome,
                }
            };
            if let Err(error) = restored { message.push_str(&format!("; restoring {name}: {error}")); }
        }
        result(plan,"failed",&message);let _=restart(&plan.root,app);return Err(message.into());
    }
    result(plan,"installed","Update installed; settings preserved.");restart(&plan.root,app)
}
fn run()->Result<()> {
    let mut args=std::env::args_os().skip(1);let path=PathBuf::from(args.next().ok_or("Missing plan")?);
    let validate_only=args.any(|a|a=="--validate-only");
    let stage=path.parent().ok_or("Missing stage")?;
    let plan:Plan=serde_json::from_slice(&fs::read(&path)?)?;
    let app = [APP, LEGACY_APP].into_iter().find(|name| plan.root.join(name).is_file())
        .ok_or("Missing installed application")?;
    let validation=validate(stage,&plan,app);
    let names=match validation {Ok(n)=>n,Err(e)=>{fs::write(stage.join("error.txt"),e.to_string())?;return Err(e)}};
    if validate_only {return Ok(())}
    fs::write(stage.join("ready.flag"),b"ready")?;
    wait_parent(plan.parent)?;
    std::thread::sleep(Duration::from_secs(2));
    apply(stage,&plan,&names,app)
}
fn main(){if let Err(error)=run(){
    if let Some(path)=std::env::args_os().nth(1).map(PathBuf::from).and_then(|p|p.parent().map(Path::to_path_buf)){let _=fs::write(path.join("error.txt"),error.to_string());}
    std::process::exit(1);
}}
