//! ViewIt Android Native Plugin for Font Metadata.
//!
//! This crate provides JNI bindings for the font-universal parser.

use jni::objects::{JClass, JObject, JString};
use jni::sys::{jboolean, jstring};
use jni::JNIEnv;
use viewit_core_types::Format;
use viewit_fmt_font::parse;

/// Initialize the plugin (no-op for now)
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_fontuniversal_FontUniversalPlugin_initialize(
    _env: JNIEnv,
    _class: JClass,
    _context: JObject,
) {
}

/// Check if the plugin can handle a given MIME type
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_fontuniversal_FontUniversalPlugin_nativeCanHandleMimeType(
    mut env: JNIEnv,
    _class: JClass,
    mime_type: JString,
) -> jboolean {
    let mime: String = env
        .get_string(&mime_type)
        .expect("Couldn't get java string!")
        .into();
    let can_handle = matches!(
        mime.as_str(),
        "font/ttf"
            | "font/otf"
            | "font/woff"
            | "font/woff2"
            | "font/sfnt"
            | "font/collection"
            | "application/x-font-ttf"
            | "application/x-font-otf"
            | "application/font-sfnt"
            | "application/vnd.ms-opentype"
            | "application/font-woff"
            | "application/font-woff2"
            | "application/x-font-truetype"
            | "application/x-font-opentype"
            | "application/x-font-woff"
    );
    can_handle as jboolean
}

/// Check if the plugin can handle a given file extension
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_fontuniversal_FontUniversalPlugin_nativeCanHandleExt(
    mut env: JNIEnv,
    _class: JClass,
    ext: JString,
) -> jboolean {
    let ext: String = env
        .get_string(&ext)
        .expect("Couldn't get java string!")
        .into();
    // Honest set: only formats the parser can actually decode.
    let can_handle = matches!(
        ext.to_lowercase().as_str(),
        "ttf" | "otf" | "woff" | "ttc"
    );
    can_handle as jboolean
}

/// Render a file and return JSON string
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_fontuniversal_FontUniversalPlugin_nativeRender(
    mut env: JNIEnv,
    _class: JClass,
    file_path: JString,
    ext: JString,
) -> jstring {
    let file_path: String = env
        .get_string(&file_path)
        .expect("Couldn't get java string!")
        .into();
    let ext: String = env
        .get_string(&ext)
        .expect("Couldn't get java string!")
        .into();

    let bytes = match std::fs::read(&file_path) {
        Ok(b) => b,
        Err(e) => {
            let error_json = format!(
                r#"{{"kind":"unsupported","format":"{}","reason":"Failed to read file: {}","suggestion":"none"}}"#,
                ext, e
            );
            return env
                .new_string(error_json)
                .expect("Couldn't create java string!")
                .into_raw();
        }
    };

    env.new_string(render_json(&bytes, &ext))
        .expect("Couldn't create java string!")
        .into_raw()
}

/// Render bytes directly and return JSON string
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_fontuniversal_FontUniversalPlugin_nativeRenderBytes(
    mut env: JNIEnv,
    _class: JClass,
    bytes: jni::objects::JByteArray,
    ext: JString,
) -> jstring {
    let ext: String = env
        .get_string(&ext)
        .expect("Couldn't get java string!")
        .into();
    let bytes_vec: Vec<u8> = env
        .convert_byte_array(&bytes)
        .expect("Couldn't convert byte array!");

    env.new_string(render_json(&bytes_vec, &ext))
        .expect("Couldn't create java string!")
        .into_raw()
}

fn render_json(bytes: &[u8], ext: &str) -> String {
    match parse(bytes, Format::Font, "") {
        Ok(doc) => serde_json::to_string(&doc)
            .unwrap_or_else(|_| unsupported_json(ext, "JSON serialization failed")),
        Err(e) => unsupported_json(ext, &format!("font parse: {}", e)),
    }
}

fn unsupported_json(ext: &str, reason: &str) -> String {
    format!(
        r#"{{"kind":"unsupported","format":"{}","reason":"{}","suggestion":"none"}}"#,
        ext,
        reason.replace('"', "\\\"")
    )
}

/// Get supported formats as JSON array
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_fontuniversal_FontUniversalPlugin_getSupportedFormats(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let formats = vec!["ttf", "otf", "woff", "ttc"];
    let json = serde_json::to_string(&formats).unwrap_or_else(|_| "[]".to_string());
    env.new_string(json)
        .expect("Couldn't create java string!")
        .into_raw()
}

/// Get plugin version
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_fontuniversal_FontUniversalPlugin_getVersion(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let version = env!("CARGO_PKG_VERSION");
    env.new_string(version)
        .expect("Couldn't create java string!")
        .into_raw()
}

/// Get plugin ID
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_fontuniversal_FontUniversalPlugin_getId(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    env.new_string("font-universal")
        .expect("Couldn't create java string!")
        .into_raw()
}

/// Cleanup (no-op for now)
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_fontuniversal_FontUniversalPlugin_nativeCleanup(
    _env: JNIEnv,
    _class: JClass,
) {
}
