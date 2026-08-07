//! JNI bindings for the office-universal Android plugin.

use jni::objects::{JClass, JString, JObject};
use jni::sys::{jstring, jboolean};
use jni::JNIEnv;
use viewit_fmt_office::render;

/// Initialize the plugin (no-op for now)
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_officeuniversal_OfficeUniversalPlugin_initialize(
    _env: JNIEnv,
    _class: JClass,
    _context: JObject,
) {
    // Plugin initialization if needed
}

/// Check if the plugin can handle a given MIME type
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_officeuniversal_OfficeUniversalPlugin_canHandleMimeType(
    mut env: JNIEnv,
    _class: JClass,
    mime_type: JString,
) -> jboolean {
    let mime: String = env.get_string(&mime_type).expect("Couldn't get java string!").into();
    let can_handle = matches!(
        mime.as_str(),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" // docx
        | "application/vnd.ms-word.document.macroEnabled.12" // docm
        | "application/vnd.openxmlformats-officedocument.wordprocessingml.template" // dotx
        | "application/vnd.ms-word.template.macroEnabled.12" // dotm
        | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" // xlsx
        | "application/vnd.ms-excel.sheet.macroEnabled.12" // xlsm
        | "application/vnd.ms-excel.sheet.binary.macroEnabled.12" // xlsb
        | "application/vnd.ms-excel" // xls
        | "application/vnd.openxmlformats-officedocument.presentationml.presentation" // pptx
        | "application/vnd.ms-powerpoint.presentation.macroEnabled.12" // pptm
        | "application/vnd.openxmlformats-officedocument.presentationml.template" // potx
        | "application/vnd.oasis.opendocument.text" // odt
        | "application/vnd.oasis.opendocument.text-template" // ott
        | "application/vnd.oasis.opendocument.spreadsheet" // ods
        | "application/vnd.oasis.opendocument.spreadsheet-template" // ots
        | "application/vnd.oasis.opendocument.presentation" // odp
        | "application/vnd.oasis.opendocument.presentation-template" // otp
        | "application/msword" // doc
        | "application/vnd.ms-powerpoint" // ppt
    );
    can_handle as jboolean
}

/// Check if the plugin can handle a given file extension
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_officeuniversal_OfficeUniversalPlugin_canHandleExt(
    mut env: JNIEnv,
    _class: JClass,
    ext: JString,
) -> jboolean {
    let ext: String = env.get_string(&ext).expect("Couldn't get java string!").into();
    let can_handle = matches!(
        ext.to_lowercase().as_str(),
        "docx" | "docm" | "dotx" | "dotm"
        | "xlsx" | "xlsm" | "xlsb" | "xls"
        | "pptx" | "pptm" | "potx"
        | "odt" | "ott"
        | "ods" | "ots"
        | "odp" | "otp"
        | "doc" | "ppt"
    );
    can_handle as jboolean
}

/// Render a file and return JSON string
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_officeuniversal_OfficeUniversalPlugin_render(
    mut env: JNIEnv,
    _class: JClass,
    file_path: JString,
    ext: JString,
) -> jstring {
    let file_path: String = env.get_string(&file_path).expect("Couldn't get java string!").into();
    let ext: String = env.get_string(&ext).expect("Couldn't get java string!").into();

    // Read file bytes
    let bytes = match std::fs::read(&file_path) {
        Ok(b) => b,
        Err(e) => {
            let error_json = format!(r#"{{"kind":"unsupported","format":"{}","reason":"Failed to read file: {}","suggestion":"none"}}"#, ext, e);
            return env.new_string(error_json).expect("Couldn't create java string!").into_raw();
        }
    };

    // Render using the unified parser
    let result = render(&bytes, &ext.to_lowercase());

    match result {
        Ok(json) => env.new_string(json).expect("Couldn't create java string!").into_raw(),
        Err(e) => {
            let error_json = format!(r#"{{"kind":"unsupported","format":"{}","reason":"{}","suggestion":"none"}}"#, ext, e);
            env.new_string(error_json).expect("Couldn't create java string!").into_raw()
        }
    }
}

/// Get supported formats as JSON array
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_officeuniversal_OfficeUniversalPlugin_getSupportedFormats(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let formats = vec![
        "docx", "docm", "dotx", "dotm",
        "xlsx", "xlsm", "xlsb", "xls",
        "pptx", "pptm", "potx",
        "odt", "ott",
        "ods", "ots",
        "odp", "otp",
        "doc", "ppt",
    ];
    let json = serde_json::to_string(&formats).unwrap_or_else(|_| "[]".to_string());
    env.new_string(json).expect("Couldn't create java string!").into_raw()
}

/// Get plugin version
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_officeuniversal_OfficeUniversalPlugin_getVersion(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let version = env!("CARGO_PKG_VERSION");
    env.new_string(version).expect("Couldn't create java string!").into_raw()
}

/// Get plugin ID
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_officeuniversal_OfficeUniversalPlugin_getId(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    env.new_string("office-universal").expect("Couldn't create java string!").into_raw()
}

/// Cleanup (no-op for now)
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_officeuniversal_OfficeUniversalPlugin_cleanup(
    _env: JNIEnv,
    _class: JClass,
) {
    // Cleanup if needed
}