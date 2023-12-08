use minijinja::{context, Environment};
use serde_json::json;

const DEFAULT_CHAT_TEMPLATE: &str = "{% for message in messages %}{{'<|im_start|>' + message['role'] + '\n' + message['content'] + '<|im_end|>' + '\n'}}{% endfor %}";
const LLAMA_TEMPLATE: &str =
    "<s>[INST] {% if system %}<<SYS>>\n{{system}}\n<</SYS>>\n\n{% endif %}{{prompt}} [/INST] ";

pub struct ChatTemplate<'a> {
    pub template: &'a str,
    pub stop: Option<&'static [&'static str]>,
    /// true to use messages as an array like the normal chat templates
    /// false to use "system" and "prompt" in the context
    pub message_array: bool,
}

pub fn builtin_chat_template(name: &str) -> Option<ChatTemplate> {
    match name {
        "llama" => Some(ChatTemplate {
            template: LLAMA_TEMPLATE,
            stop: Some(&["</s>"]),
            message_array: false,
        }),
        "default" => Some(ChatTemplate {
            template: DEFAULT_CHAT_TEMPLATE,
            stop: None,
            message_array: true,
        }),
        _ => None,
    }
}

pub fn apply_chat_template(
    template: ChatTemplate,
    prompt: &str,
    system: Option<&str>,
    add_generation_prompt: bool,
) -> Result<String, minijinja::Error> {
    let mut env = Environment::new();
    env.add_template("template", template.template)?;

    let context = if template.message_array {
        let mut messages = vec![];
        if let Some(system) = system {
            messages.push(json!({
                "role": "system",
                "content": system
            }));
        }

        messages.push(json!({
            "role": "user",
            "content": prompt
        }));

        context!(
            messages => messages,
            add_generation_prompt => add_generation_prompt,
        )
    } else {
        context!(
            system => system,
            prompt => prompt,
        )
    };

    let tmpl = env
        .get_template("template")
        .expect("Just-added template was not found");
    let output = tmpl.render(context)?;

    if add_generation_prompt {
        // This isn't great but for Together,
        Ok(format!("{output}<|im_start|>assistant\n"))
    } else {
        Ok(output)
    }
}

#[cfg(test)]
mod test {
    use super::{apply_chat_template, builtin_chat_template};

    #[test]
    fn default_chat_template_with_system() {
        let template = builtin_chat_template("default").unwrap();
        let result = apply_chat_template(template, "hello", Some("sys prompt"), true).unwrap();
        assert_eq!(result, "<|im_start|>system\nsys prompt<|im_end|>\n<|im_start|>user\nhello<|im_end|>\n<|im_start|>assistant\n");
    }

    #[test]
    fn default_chat_template_without_system() {
        let template = builtin_chat_template("default").unwrap();
        let result = apply_chat_template(template, "hello", None, true).unwrap();
        assert_eq!(
            result,
            "<|im_start|>user\nhello<|im_end|>\n<|im_start|>assistant\n"
        );
    }

    #[test]
    fn llama_chat_template_with_system() {
        let template = builtin_chat_template("llama").unwrap();
        let result = apply_chat_template(template, "hello", Some("sys prompt"), false).unwrap();
        assert_eq!(
            result,
            "<s>[INST] <<SYS>>\nsys prompt\n<</SYS>>\n\nhello [/INST] "
        );
    }

    #[test]
    fn llama_chat_template_without_system() {
        let template = builtin_chat_template("llama").unwrap();
        let result = apply_chat_template(template, "hello", None, false).unwrap();
        assert_eq!(result, "<s>[INST] hello [/INST] ");
    }
}
