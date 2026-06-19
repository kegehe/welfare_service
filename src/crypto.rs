use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    AeadCore, Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

use crate::error::{AppError, Result};

/// AES-256-GCM 加密存储
///
/// 密文格式: [12 bytes nonce][ciphertext + 16 bytes tag]
/// 存储编码: base64
#[derive(Clone)]
pub struct KeyStore {
    cipher: Aes256Gcm,
}

impl KeyStore {
    /// 从 base64 编码的 32 字节主密钥创建
    pub fn from_base64_key(key_b64: &str) -> Result<Self> {
        let key_bytes = BASE64
            .decode(key_b64)
            .map_err(|e| AppError::Crypto(format!("base64 解码主密钥失败: {}", e)))?;
        if key_bytes.len() != 32 {
            return Err(AppError::Crypto(format!(
                "主密钥长度错误: 期望 32 字节, 实际 {} 字节",
                key_bytes.len()
            )));
        }
        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|e| AppError::Crypto(format!("创建 AES-256-GCM 实例失败: {}", e)))?;
        Ok(Self { cipher })
    }

    /// 生成一个新的随机 32 字节主密钥 (base64 编码)
    pub fn generate_key() -> String {
        use rand::RngCore;
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        BASE64.encode(key)
    }

    /// 加密明文，返回 base64 编码的密文
    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| AppError::Crypto(format!("加密失败: {}", e)))?;

        // 拼接 nonce + ciphertext
        let mut combined = Vec::with_capacity(12 + ciphertext.len());
        combined.extend_from_slice(nonce.as_slice());
        combined.extend_from_slice(&ciphertext);

        Ok(BASE64.encode(combined))
    }

    /// 解密 base64 编码的密文
    pub fn decrypt(&self, encrypted_b64: &str) -> Result<String> {
        let combined = BASE64
            .decode(encrypted_b64)
            .map_err(|e| AppError::Crypto(format!("base64 解码密文失败: {}", e)))?;

        if combined.len() < 12 {
            return Err(AppError::Crypto("密文数据过短".to_string()));
        }

        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| AppError::Crypto(format!("解密失败: {}", e)))?;

        String::from_utf8(plaintext)
            .map_err(|e| AppError::Crypto(format!("解密结果不是有效 UTF-8: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key_b64 = KeyStore::generate_key();
        let store = KeyStore::from_base64_key(&key_b64).unwrap();

        let plaintext = "sk-test-api-key-12345";
        let encrypted = store.encrypt(plaintext).unwrap();
        let decrypted = store.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_different_nonces() {
        let key_b64 = KeyStore::generate_key();
        let store = KeyStore::from_base64_key(&key_b64).unwrap();

        let e1 = store.encrypt("same").unwrap();
        let e2 = store.encrypt("same").unwrap();

        // 同明文不同密文 (因为 nonce 不同)
        assert_ne!(e1, e2);
        // 但都能正确解密
        assert_eq!(store.decrypt(&e1).unwrap(), "same");
        assert_eq!(store.decrypt(&e2).unwrap(), "same");
    }

    #[test]
    fn test_invalid_key_length() {
        let result = KeyStore::from_base64_key("dGVzdA=="); // "test" = 4 bytes
        assert!(result.is_err());
    }
}
