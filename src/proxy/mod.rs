pub mod forwarder;
pub mod orchestrator;
pub mod stream;

/// 协议类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// OpenAI 兼容格式: /v1/chat/completions
    OpenAI,
    /// Claude Messages 格式: /v1/messages
    Claude,
}

/// 构建上游 URL，自动去除 base_url 末尾与请求路径开头的重复路径段
///
/// 用户配置 openai_url 时可能包含 `/v1`（如 "https://api.example.com/v1"），
/// 也可能不含（如 "https://api.example.com"）。请求路径总是 `/v1/chat/completions`
/// 或 `/v1/messages`。如果 base_url 末尾和请求路径开头有重复的**完整路径段**，
/// 需要去除。
///
/// 例如:
///   base_url = "https://api.example.com/v1"
///   request_path = "/v1/chat/completions"
///   -> 去除重复 "/v1"，结果 = "https://api.example.com/v1/chat/completions"
///
///   base_url = "https://api.example.com"
///   request_path = "/v1/chat/completions"
///   -> 无重复，结果 = "https://api.example.com/v1/chat/completions"
///
/// 只检测以 `/` 开头的完整路径段，避免域名中的子串（如 `.com`）误匹配。
pub fn build_upstream_url(base_url: &str, request_path: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');

    // 寻找 request_path 中最长的、以 '/' 开头的前缀，且 base 以它结尾
    // 从 request_path 中找到所有 '/' 的位置，从最后一个开始检查
    // 这样只匹配完整的路径段（如 /v1, /v1/chat），不会匹配 /v1/c 等部分段
    for (i, c) in request_path.char_indices().rev() {
        if c != '/' {
            continue;
        }
        if i == 0 {
            // 跳过 request_path 本身的开头 '/'（否则所有 base 都会匹配 "/"）
            continue;
        }
        let prefix = &request_path[..i];
        if base.ends_with(prefix) {
            // base 末尾和 request_path 开头重合了 prefix 部分
            // 拼接时保留 base（含重复部分），只追加 request_path 中不重合的部分
            let remaining_path = &request_path[i..];
            return format!("{}{}", base, remaining_path);
        }
    }

    // 无重复，直接拼接
    format!("{}{}", base, request_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_overlap() {
        assert_eq!(
            build_upstream_url("https://api.example.com", "/v1/chat/completions"),
            "https://api.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn test_v1_overlap() {
        assert_eq!(
            build_upstream_url("https://api.example.com/v1", "/v1/chat/completions"),
            "https://api.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn test_trailing_slash() {
        assert_eq!(
            build_upstream_url("https://api.example.com/v1/", "/v1/chat/completions"),
            "https://api.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn test_claude_path() {
        assert_eq!(
            build_upstream_url("https://api.example.com", "/v1/messages"),
            "https://api.example.com/v1/messages"
        );
    }

    #[test]
    fn test_claude_path_overlap() {
        assert_eq!(
            build_upstream_url("https://api.example.com/v1", "/v1/messages"),
            "https://api.example.com/v1/messages"
        );
    }

    #[test]
    fn test_anthropic_no_overlap() {
        assert_eq!(
            build_upstream_url("https://api.anthropic.com", "/v1/messages"),
            "https://api.anthropic.com/v1/messages"
        );
    }

    #[test]
    fn test_xiaomimimo() {
        assert_eq!(
            build_upstream_url(
                "https://token-plan-cn.xiaomimimo.com/v1",
                "/v1/chat/completions"
            ),
            "https://token-plan-cn.xiaomimimo.com/v1/chat/completions"
        );
    }

    #[test]
    fn test_base_with_leading_space() {
        // 前导空格会被 trim 掉
        assert_eq!(
            build_upstream_url(" https://api.example.com/v1", "/v1/chat/completions"),
            "https://api.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn test_no_false_positive_domain() {
        // 域名中的 .com 不应该和 /com 开头的路径误匹配
        assert_eq!(
            build_upstream_url("https://api.example.com", "/v1/chat/completions"),
            "https://api.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn test_iflytek_claude() {
        // 讯飞的 Claude 兼容 URL (无 /v1 重叠)
        assert_eq!(
            build_upstream_url(
                "https://maas-coding-api.cn-huabei-1.xf-yun.com/anthropic",
                "/v1/messages"
            ),
            "https://maas-coding-api.cn-huabei-1.xf-yun.com/anthropic/v1/messages"
        );
    }

    #[test]
    fn test_deep_overlap() {
        // 更深层的重复: base 含 /v1/chat
        assert_eq!(
            build_upstream_url("https://api.example.com/v1/chat", "/v1/chat/completions"),
            "https://api.example.com/v1/chat/completions"
        );
    }
}
