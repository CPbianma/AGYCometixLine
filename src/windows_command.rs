//! Windows AGY shell commands must not begin with a quoted executable.
//! Use direct paths when possible; an encoded UTF-8 bridge handles spaces.

const ARGUMENTS: &str = " -NoLogo -NoProfile -NonInteractive -EncodedCommand ";
const MARKER: &str = "# agyline native launcher\n";

fn utf16_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::new();
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        encoded.push(ALPHABET[(first >> 2) as usize] as char);
        encoded.push(ALPHABET[((first & 3) << 4 | second >> 4) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            ALPHABET[((second & 15) << 2 | third >> 6) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            ALPHABET[(third & 63) as usize] as char
        } else {
            '='
        });
    }
    encoded
}

pub fn for_executable(path: &str) -> Result<String, String> {
    let powershell = system_powershell()?;
    let encoded_path = base64(&utf16_bytes(path));
    let script = format!(
        "{MARKER}$utf8 = [System.Text.UTF8Encoding]::new($false); \
         [Console]::InputEncoding = $utf8; [Console]::OutputEncoding = $utf8; \
         $OutputEncoding = $utf8; \
         $executable = [System.Text.Encoding]::Unicode.GetString([Convert]::FromBase64String('{encoded_path}')); \
         [Console]::In.ReadToEnd() | & $executable; exit $LASTEXITCODE"
    );
    let command = format!("{powershell}{ARGUMENTS}{}", base64(&utf16_bytes(&script)));
    if command.len() >= 8191 {
        return Err("encoded Windows launcher exceeds the host shell command limit".into());
    }
    Ok(command)
}

#[cfg(windows)]
fn system_powershell() -> Result<String, String> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetSystemDirectoryW(buffer: *mut u16, size: u32) -> u32;
    }
    let mut buffer = vec![0u16; 32768];
    // Resolve via the OS, never the working directory, PATH or an env override.
    let length = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) } as usize;
    if length == 0 || length >= buffer.len() {
        return Err("cannot resolve the Windows system directory".into());
    }
    let directory = String::from_utf16(&buffer[..length]).map_err(|error| error.to_string())?;
    let executable = format!("{directory}\\WindowsPowerShell\\v1.0\\powershell.exe");
    if executable.chars().any(char::is_whitespace) || !std::path::Path::new(&executable).is_file() {
        return Err("system PowerShell is unavailable at an unquoted absolute path; choose an install path without spaces".into());
    }
    Ok(executable)
}

#[cfg(not(windows))]
fn system_powershell() -> Result<String, String> {
    Err("the encoded launcher is only available on Windows".into())
}

/// A connection-label hint only; installation ownership uses the saved record.
pub fn recognises(command: &str) -> bool {
    let Some((executable, encoded)) = command.split_once(ARGUMENTS) else {
        return false;
    };
    if !executable
        .to_ascii_lowercase()
        .ends_with("\\windowspowershell\\v1.0\\powershell.exe")
    {
        return false;
    }
    let marker = utf16_bytes(MARKER);
    let complete_groups = marker.len() / 3 * 3;
    encoded.starts_with(&base64(&marker[..complete_groups]))
        && encoded
            .bytes()
            .all(|character| character.is_ascii_alphanumeric() || b"+/=".contains(&character))
}
