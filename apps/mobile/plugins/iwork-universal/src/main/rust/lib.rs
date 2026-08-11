use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{jboolean, jstring};
use jni::JNIEnv;
use viewit_core_types::Format;

fn format_for(ext: &str) -> Option<Format> {
    match ext.to_ascii_lowercase().as_str() {
        "pages" => Some(Format::IworkPages),
        "numbers" => Some(Format::IworkNumbers),
        "key" => Some(Format::IworkKey),
        _ => None,
    }
}
fn unsupported(ext: &str, reason: &str) -> String {
    serde_json::json!({"kind":"unsupported","format":ext,"reason":reason,"suggestion":"none"}).to_string()
}
fn render_json(bytes: &[u8], ext: &str) -> String {
    let Some(format)=format_for(ext) else { return unsupported(ext,"unsupported iWork extension"); };
    match viewit_fmt_iwork::parse(bytes, format, "") {
        Ok(doc)=>serde_json::to_string(&doc).unwrap_or_else(|e|unsupported(ext,&e.to_string())),
        Err(e)=>unsupported(ext,&format!("iwork parse: {e}")),
    }
}
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_iworkuniversal_IworkUniversalPlugin_nativeCanHandleExt(mut env:JNIEnv,_:JClass,ext:JString)->jboolean{
    let s:String=env.get_string(&ext).map(|s|s.into()).unwrap_or_default(); format_for(&s).is_some() as jboolean
}
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_iworkuniversal_IworkUniversalPlugin_nativeCanHandleMimeType(mut env:JNIEnv,_:JClass,m:JString)->jboolean{
    let s:String=env.get_string(&m).map(|s|s.into()).unwrap_or_default(); (s.contains("iwork")||s.contains("pages")||s.contains("numbers")||s.contains("keynote")) as jboolean
}
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_iworkuniversal_IworkUniversalPlugin_nativeRenderBytes(mut env:JNIEnv,_:JClass,bytes:JByteArray,ext:JString)->jstring{
    let e:String=env.get_string(&ext).map(|s|s.into()).unwrap_or_default(); let b=env.convert_byte_array(bytes).unwrap_or_default();
    env.new_string(render_json(&b,&e)).expect("jni string").into_raw()
}
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_iworkuniversal_IworkUniversalPlugin_nativeCleanup(_:JNIEnv,_:JClass){}
