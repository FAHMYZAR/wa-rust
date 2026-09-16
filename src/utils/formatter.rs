/// WhatsApp markdown text formatter helpers matching botwanewja Formatter.
pub struct Formatter;

#[allow(dead_code)]
impl Formatter {
    pub fn bold(text: &str) -> String {
        format!("*{text}*")
    }

    pub fn italic(text: &str) -> String {
        format!("_{text}_")
    }

    pub fn strike(text: &str) -> String {
        format!("~{text}~")
    }

    pub fn code(text: &str) -> String {
        format!("`{text}`")
    }

    pub fn code_block(text: &str, lang: &str) -> String {
        format!("```{lang}\n{text}\n```")
    }

    pub fn quote(text: &str) -> String {
        format!("> {text}")
    }

    pub fn list(items: &[String]) -> String {
        items
            .iter()
            .map(|item| format!("• {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn section(title: &str) -> String {
        format!("\n> *{title}*")
    }
}
