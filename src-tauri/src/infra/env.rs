//! .env 加载：exe 同目录优先（打包态），其次 cwd 向上查找（开发态命中项目根）。
//! 不覆盖已存在的系统环境变量；文件缺失静默跳过，解析失败仅 stderr 提示、不挡启动。

/// 启动第一行调用，先于单实例锁与 Store::load()。
pub fn load() {
  if let Ok(exe) = std::env::current_exe() {
    if let Some(dir) = exe.parent() {
      let path = dir.join(".env");
      if path.exists() {
        if let Err(error) = dotenvy::from_path(&path) {
          eprintln!(".env 解析失败（{}）：{error}", path.display());
        }
      }
    }
  }
  match dotenvy::dotenv() {
    Err(error) if !error.not_found() => eprintln!(".env 解析失败：{error}"),
    _ => {}
  }
}