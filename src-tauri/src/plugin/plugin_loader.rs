use std::{
    collections::HashMap,
    ffi::CStr,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, Weak},
};

use bilibili_video_downloader_plugin_api::{SDK_API_VERSION_V1, v1::PluginDescriptorV1};
use dlopen2::wrapper::Container;
use eyre::{WrapErr, eyre};
use parking_lot::Mutex;
use tracing::instrument;

use crate::plugin::{
    host_api,
    plugin_types::{PluginDylibApi, PluginRuntime},
};

// Loading the same native library again can reuse its process-global buffers.
// Share a gate across runtime replacements as well as ordinary runtime clones.
static CALL_LOCKS: LazyLock<Mutex<HashMap<PathBuf, Weak<Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn call_lock_for(path: &Path) -> eyre::Result<Arc<Mutex<()>>> {
    let path = std::fs::canonicalize(path)?;
    let mut locks = CALL_LOCKS.lock();
    locks.retain(|_, lock| lock.strong_count() > 0);
    if let Some(lock) = locks.get(&path).and_then(Weak::upgrade) {
        return Ok(lock);
    }
    let lock = Arc::new(Mutex::new(()));
    locks.insert(path, Arc::downgrade(&lock));
    Ok(lock)
}

#[instrument(level = "error", skip_all, fields(plugin_path = %plugin_path.display(), priority = priority, enabled = enabled))]
pub fn load_plugin_from_path(
    plugin_path: &Path,
    priority: i32,
    enabled: bool,
) -> eyre::Result<PluginRuntime> {
    if !plugin_path.is_absolute() {
        return Err(eyre!("插件路径必须是绝对路径: `{}`", plugin_path.display()));
    }
    if !plugin_path.exists() {
        return Err(eyre!("插件动态库文件`{}`不存在", plugin_path.display()));
    }

    let call_lock = call_lock_for(plugin_path)?;
    let loading = call_lock.lock();

    // SAFETY: loading a native plugin executes trusted native code. Its exported
    // symbols must implement PluginDylibApi; the Container owns the library lifetime.
    let api = unsafe { Container::<PluginDylibApi>::load(plugin_path) }
        .wrap_err(format!("加载插件动态库文件`{}`失败", plugin_path.display()))?;

    let descriptor_json = get_descriptor_json(&api).wrap_err("读取插件描述失败")?;
    let descriptor: PluginDescriptorV1 = serde_json::from_str(&descriptor_json)
        .wrap_err(format!("解析插件描述失败: {descriptor_json}"))?;

    if descriptor.sdk_api_version != SDK_API_VERSION_V1 {
        return Err(eyre!(
            "插件SDK版本不匹配: 期望版本={}, 实际版本={}",
            SDK_API_VERSION_V1,
            descriptor.sdk_api_version
        ));
    }

    if descriptor.id.trim().is_empty() {
        return Err(eyre!("descriptor.id 为空"));
    }

    if descriptor.hooks.is_empty() {
        return Err(eyre!("插件未声明任何可执行 Hook"));
    }

    let host_api = host_api::build_host_api_v1();
    // SAFETY: V1 requires the plugin to copy this table during the call; it must
    // not retain the stack pointer. The function pointers themselves are static.
    let rc = unsafe { api.set_host_api(&raw const host_api) };
    if rc != 0 {
        return Err(eyre!(
            "注册宿主 Host API 失败: plugin_id={}, rc={rc}",
            descriptor.id
        ));
    }

    drop(loading);
    Ok(PluginRuntime {
        descriptor,
        plugin_path: plugin_path.to_path_buf(),
        enabled,
        priority,
        api: Arc::new(api),
        call_lock,
    })
}

#[instrument(level = "error", skip_all)]
fn get_descriptor_json(api: &Container<PluginDylibApi>) -> eyre::Result<String> {
    // SAFETY: the trusted V1 entry point belongs to the live Container.
    let descriptor_ptr = unsafe { api.descriptor() };
    if descriptor_ptr.is_null() {
        return Err(eyre!("descriptor 指针为空"));
    }

    // SAFETY: the trusted ABI promises a library-owned, NUL-terminated descriptor
    // for the library's lifetime. Null was checked; UTF-8 is checked below.
    let descriptor_cstr = unsafe { CStr::from_ptr(descriptor_ptr).to_str() }
        .wrap_err("descriptor 非 UTF-8 字符串")?;

    Ok(descriptor_cstr.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reloads_and_path_aliases_share_the_native_call_gate() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("plugin.dll");
        std::fs::write(&path, []).unwrap();
        let first = call_lock_for(&path).unwrap();
        let alias = directory.path().join(".").join("plugin.dll");
        let replacement = call_lock_for(&alias).unwrap();
        assert!(Arc::ptr_eq(&first, &replacement));
        let weak = Arc::downgrade(&first);
        drop(first);
        drop(replacement);
        assert!(weak.upgrade().is_none());
        assert!(call_lock_for(&path).is_ok());
    }
}
