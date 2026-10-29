//! 当前用户显式注册；只管理 Cloudleaf 自己的 CLSID、profile 与类别。
use super::{LANGUAGE, PROFILE_ID, SERVICE_ID};
use std::io::{Read, Write};
use std::{ffi::OsStr, os::windows::ffi::OsStrExt, path::Path};
use windows::Win32::{
    Foundation::*,
    System::{Com::*, Registry::*},
    UI::{Input::KeyboardAndMouse::HKL, TextServices::*},
};
use windows_core::{Error, PCWSTR, Result};

const KEY: &str = "Software\\Classes\\CLSID\\{F573E0D6-2C67-4FD8-A75A-7A58278DC1B0}";
fn wide(value: impl AsRef<OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain([0]).collect()
}
struct Apartment;
impl Apartment {
    fn new() -> Result<Self> {
        // 安全：管理进程的主线程独占初始化并成对 CoUninitialize。
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        }
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        // 安全：只匹配本对象成功的初始化。
        unsafe {
            CoUninitialize();
        }
    }
}
fn value(path: &str, name: Option<&str>, text: &str) -> Result<()> {
    let p = wide(path);
    let n = name.map(wide);
    let t = wide(text);
    let mut key = HKEY::default();
    // 安全：UTF16 缓冲区有效且以零结尾，创建 HKCU 内准确路径。
    unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(p.as_ptr()),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut key,
            None,
        )
        .ok()?;
        let bytes = std::slice::from_raw_parts(t.as_ptr().cast::<u8>(), t.len() * 2);
        let result = RegSetValueExW(
            key,
            n.as_ref()
                .map(|v| PCWSTR(v.as_ptr()))
                .unwrap_or(PCWSTR::null()),
            None,
            REG_SZ,
            Some(bytes),
        )
        .ok();
        let _ = RegCloseKey(key);
        result
    }
}
fn remove_com() -> Result<()> {
    let p = wide(KEY);
    // 安全：常量路径仅指向本服务，递归删除不会越过该 CLSID。
    unsafe {
        let status = RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(p.as_ptr()));
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        status.ok()
    }
}
fn profiles() -> Result<ITfInputProcessorProfiles> {
    // 安全：管理线程已初始化 COM；系统类由 Windows 提供。
    unsafe { CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER) }
}
fn categories() -> Result<ITfCategoryMgr> {
    // 安全：同上。
    unsafe { CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER) }
}
fn register(dll: &Path) -> Result<()> {
    if !dll.is_absolute() || !dll.is_file() {
        return Err(Error::from_hresult(E_INVALIDARG));
    }
    let path = dll
        .to_str()
        .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
    let p = profiles()?;
    let c = categories()?;
    if registered_com()? {
        return Err(Error::new(
            windows_core::HRESULT::from_win32(ERROR_ALREADY_EXISTS.0),
            "已有 Cloudleaf COM 注册，请先检查并显式卸载",
        ));
    }
    // 安全：枚举系统已登记服务，只比较 GUID，不读取用户输入。
    unsafe {
        let enumerator = p.EnumInputProcessorInfo()?;
        loop {
            let mut ids = [windows_core::GUID::zeroed()];
            let mut fetched = 0;
            enumerator.Next(&mut ids, Some(&mut fetched)).ok()?;
            if fetched == 0 {
                break;
            }
            if ids[0] == SERVICE_ID {
                return Err(Error::new(E_FAIL, "已有同 GUID 的 TSF 服务，拒绝覆盖"));
            }
        }
    }
    reserve_com()?;
    let mut registered = false;
    let mut profile = false;
    let mut keyboard = false;
    let mut ui = false;
    let result = (|| {
        value(KEY, None, "Cloudleaf 自研原型输入服务")?;
        value(&format!("{KEY}\\InprocServer32"), None, path)?;
        value(
            &format!("{KEY}\\InprocServer32"),
            Some("ThreadingModel"),
            "Apartment",
        )?;
        // 安全：GUID 固定，UTF16 切片长度由绑定生成；不设默认 profile。
        unsafe {
            p.Register(&SERVICE_ID)?;
            registered = true;
            let description: Vec<u16> = "云笺 Cloudleaf 原型".encode_utf16().collect();
            p.AddLanguageProfile(&SERVICE_ID, LANGUAGE, &PROFILE_ID, &description, &[], 0)?;
            profile = true;
            p.EnableLanguageProfile(&SERVICE_ID, LANGUAGE, &PROFILE_ID, true)?;
            c.RegisterCategory(&SERVICE_ID, &GUID_TFCAT_TIP_KEYBOARD, &SERVICE_ID)?;
            keyboard = true;
            c.RegisterCategory(
                &SERVICE_ID,
                &GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
                &SERVICE_ID,
            )?;
            ui = true;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let mut failures = Vec::new();
        // 安全：只逆序撤销本次成功步骤，任何回滚失败都保留为诊断。
        unsafe {
            if ui {
                collect(
                    &mut failures,
                    c.UnregisterCategory(
                        &SERVICE_ID,
                        &GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
                        &SERVICE_ID,
                    ),
                );
            }
            if keyboard {
                collect(
                    &mut failures,
                    c.UnregisterCategory(&SERVICE_ID, &GUID_TFCAT_TIP_KEYBOARD, &SERVICE_ID),
                );
            }
            if profile {
                collect(
                    &mut failures,
                    p.RemoveLanguageProfile(&SERVICE_ID, LANGUAGE, &PROFILE_ID),
                );
            }
            if registered {
                collect(&mut failures, p.Unregister(&SERVICE_ID));
            }
        }
        collect(&mut failures, remove_com());
        return if failures.is_empty() {
            Err(error)
        } else {
            Err(Error::new(
                error.code(),
                format!("注册失败：{error}；回滚失败：{}", failures.join("; ")),
            ))
        };
    }
    Ok(())
}
fn collect(errors: &mut Vec<String>, result: Result<()>) {
    if let Err(error) = result {
        errors.push(error.to_string());
    }
}
fn reserve_com() -> Result<()> {
    let path = wide(KEY);
    let mut key = HKEY::default();
    let mut disposition = REG_CREATE_KEY_DISPOSITION::default();
    // 安全：原子新建仅自己的 CLSID，已存在时不修改、不删除该项。
    unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(path.as_ptr()),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut key,
            Some(&mut disposition),
        )
        .ok()?;
        let _ = RegCloseKey(key);
    }
    if disposition != REG_CREATED_NEW_KEY {
        return Err(Error::new(E_FAIL, "COM 注册项在检查后已存在，拒绝覆盖"));
    }
    Ok(())
}
fn unregister() -> Result<()> {
    let p = profiles()?;
    let c = categories()?;
    // 安全：只撤销此服务自己登记的两类和 profile，保留其他输入法设置。
    let mut failures = Vec::new();
    unsafe {
        collect(
            &mut failures,
            c.UnregisterCategory(&SERVICE_ID, &GUID_TFCAT_TIP_KEYBOARD, &SERVICE_ID),
        );
        collect(
            &mut failures,
            c.UnregisterCategory(
                &SERVICE_ID,
                &GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
                &SERVICE_ID,
            ),
        );
        collect(
            &mut failures,
            p.RemoveLanguageProfile(&SERVICE_ID, LANGUAGE, &PROFILE_ID),
        );
        collect(&mut failures, p.Unregister(&SERVICE_ID));
    }
    collect(&mut failures, remove_com());
    if failures.is_empty() {
        Ok(())
    } else {
        Err(Error::new(
            E_FAIL,
            format!("卸载部分失败：{}", failures.join("; ")),
        ))
    }
}
fn registered_com() -> Result<bool> {
    let p = wide(KEY.strip_prefix("Software\\Classes\\").unwrap());
    let mut key = HKEY::default();
    // 安全：只读 HKCR 合并视图，拒绝覆盖当前用户或机器已有的同 CLSID；成功打开后立即关闭。
    unsafe {
        let status = RegOpenKeyExW(
            HKEY_CLASSES_ROOT,
            PCWSTR(p.as_ptr()),
            None,
            KEY_READ,
            &mut key,
        );
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(false);
        }
        status.ok()?;
        let _ = RegCloseKey(key);
        Ok(true)
    }
}
fn profile_manager() -> Result<ITfInputProcessorProfileMgr> {
    use windows_core::Interface;
    profiles()?.cast()
}
fn snapshot(profile: TF_INPUTPROCESSORPROFILE) -> String {
    format!(
        "cloudleaf-profile-v1\n{}\n{}\n{:?}\n{:?}\n{:X}\n",
        profile.dwProfileType,
        profile.langid,
        profile.clsid,
        profile.guidProfile,
        profile.hkl.0 as usize
    )
}
fn parse_snapshot(text: &str) -> Result<TF_INPUTPROCESSORPROFILE> {
    let lines: Vec<_> = text.lines().collect();
    let invalid = || Error::from_hresult(E_INVALIDARG);
    if text.len() > 1024
        || text.contains('\0')
        || lines.len() != 6
        || lines[0] != "cloudleaf-profile-v1"
    {
        return Err(invalid());
    }
    let kind = lines[1].parse::<u32>().map_err(|_| invalid())?;
    if kind != TF_PROFILETYPE_INPUTPROCESSOR && kind != TF_PROFILETYPE_KEYBOARDLAYOUT {
        return Err(invalid());
    }
    let profile = TF_INPUTPROCESSORPROFILE {
        dwProfileType: kind,
        langid: lines[2].parse().map_err(|_| invalid())?,
        clsid: lines[3].try_into()?,
        guidProfile: lines[4].try_into()?,
        hkl: HKL(usize::from_str_radix(lines[5], 16).map_err(|_| invalid())? as *mut _),
        ..Default::default()
    };
    if profile.langid == 0
        || (kind == TF_PROFILETYPE_INPUTPROCESSOR
            && (profile.clsid == windows_core::GUID::zeroed()
                || profile.guidProfile == windows_core::GUID::zeroed()))
        || (kind == TF_PROFILETYPE_KEYBOARDLAYOUT && profile.hkl.0.is_null())
    {
        return Err(invalid());
    }
    Ok(profile)
}
fn activate_session(path: &Path) -> Result<()> {
    let manager = profile_manager()?;
    let mut old = TF_INPUTPROCESSORPROFILE::default();
    // 安全：写入本地值，系统返回当前会话的键盘类别 profile。
    unsafe {
        manager.GetActiveProfile(&GUID_TFCAT_TIP_KEYBOARD, &mut old)?;
    }
    let data = snapshot(old);
    parse_snapshot(&data)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| Error::new(E_FAIL, e.to_string()))?;
    file.write_all(data.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|e| Error::new(E_FAIL, e.to_string()))?;
    // 安全：先持久保存原 profile；只调整本登录会话，不设置默认/系统配置。
    let activated = unsafe {
        manager.ActivateProfile(
            TF_PROFILETYPE_INPUTPROCESSOR,
            LANGUAGE,
            &SERVICE_ID,
            &PROFILE_ID,
            HKL::default(),
            TF_IPPMF_FORSESSION,
        )
    };
    if let Err(error) = activated {
        // 安全：失败也尝试恢复已持久保存的旧 profile，避免部分切换遗留。
        let restored = unsafe {
            manager.ActivateProfile(
                old.dwProfileType,
                old.langid,
                &old.clsid,
                &old.guidProfile,
                old.hkl,
                TF_IPPMF_FORSESSION,
            )
        };
        return match restored {
            Ok(()) => Err(error),
            Err(restore) => Err(Error::new(
                error.code(),
                format!("激活失败：{error}；恢复亦失败：{restore}"),
            )),
        };
    }
    Ok(())
}
fn restore_session(path: &Path) -> Result<()> {
    let mut text = String::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(1025).read_to_string(&mut text))
        .map_err(|e| Error::new(E_FAIL, e.to_string()))?;
    let p = parse_snapshot(&text)?;
    // 安全：严格解析的快照参数交由 TSF 校验，恢复原 type/lang/CLSID/profile/HKL。
    unsafe {
        profile_manager()?.ActivateProfile(
            p.dwProfileType,
            p.langid,
            &p.clsid,
            &p.guidProfile,
            p.hkl,
            TF_IPPMF_FORSESSION,
        )?;
    }
    Ok(())
}
pub(crate) fn manage(args: Vec<String>) -> Result<()> {
    let _apartment = Apartment::new()?;
    match args.first().map(String::as_str) {
        Some("register") if args.len() == 2 => {
            register(Path::new(&args[1]))?;
            println!(
                "已注册 Cloudleaf，未修改默认输入法。CLSID={SERVICE_ID:?} profile={PROFILE_ID:?}"
            );
        }
        Some("unregister") if args.len() == 1 => {
            unregister()?;
            println!("已卸载 Cloudleaf 注册项");
        }
        Some("status") if args.len() == 1 => {
            if !registered_com()? {
                println!("Cloudleaf 未注册");
                return Ok(());
            }
            let p = profiles()?;
            // 安全：输出指向局部 BOOL，固定标识只查询 Cloudleaf。
            let enabled =
                unsafe { p.IsEnabledLanguageProfile(&SERVICE_ID, LANGUAGE, &PROFILE_ID)? };
            println!(
                "CLSID={SERVICE_ID:?} profile={PROFILE_ID:?} enabled={}",
                enabled.as_bool()
            );
        }
        Some("activate-session") if args.len() == 2 => {
            activate_session(Path::new(&args[1]))?;
            println!("已保存原 profile 并激活本会话 Cloudleaf；使用 restore-session 恢复");
        }
        Some("restore-session") if args.len() == 2 => {
            restore_session(Path::new(&args[1]))?;
            println!("已恢复快照中的会话 profile");
        }
        Some("activate-local") if args.len() == 1 => {
            let p: ITfInputProcessorProfileMgr = {
                use windows_core::Interface;
                profiles()?.cast()?
            };
            // 安全：只在此管理进程激活，不修改用户默认；用于诊断 TSF profile 可加载性。
            unsafe {
                p.ActivateProfile(
                    TF_PROFILETYPE_INPUTPROCESSOR,
                    LANGUAGE,
                    &SERVICE_ID,
                    &PROFILE_ID,
                    HKL::default(),
                    TF_IPPMF_FORPROCESS,
                )?;
            }
            println!("管理进程内激活完成");
        }
        _ => {
            println!(
                "cloudleaf-ime register <绝对DLL路径> | unregister | status | activate-local | activate-session <新快照路径> | restore-session <快照路径>"
            )
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_snapshot_roundtrip_preserves_full_identity() {
        let p = TF_INPUTPROCESSORPROFILE {
            dwProfileType: TF_PROFILETYPE_INPUTPROCESSOR,
            langid: 0x804,
            clsid: SERVICE_ID,
            guidProfile: PROFILE_ID,
            hkl: HKL(0xe0200804usize as *mut _),
            ..Default::default()
        };
        let restored = parse_snapshot(&snapshot(p)).unwrap();
        assert_eq!(restored.dwProfileType, p.dwProfileType);
        assert_eq!(restored.langid, p.langid);
        assert_eq!(restored.clsid, p.clsid);
        assert_eq!(restored.guidProfile, p.guidProfile);
        assert_eq!(restored.hkl, p.hkl);
    }
    #[test]
    fn malformed_snapshot_rejected() {
        for text in [
            "",
            "cloudleaf-profile-v1\n0\n1\nx\nx\n0\n",
            "cloudleaf-profile-v1\n1\n1\nx\nx\n0\n",
        ] {
            assert!(parse_snapshot(text).is_err());
        }
    }
}
