<div class="oranda-hide">

# PromptBox

</div>

PromptBox allows maintaining libraries of LLM prompt templates which can be filled in and submitted from the command
line. It can submit prompts to the OpenAI API, [Ollama](https://ollama.ai), or [LM Studio](https://lmstudio.ai/).

# Template Files

- are built in TOML
- can use Liquid templating, and reference templates in other files
- define command-line arguments, which can include references to files
- have the filename format `<NAME>.pb.toml`

```toml
# File: summarize.pb.toml

description = "Summarize some files"

# Optional system prompt
# Or `system_prompt_path` to read from a template file
system_prompt = "You are a great summarizer."

# This can also be template_path to read from another file.
template = '''
Create a {{style}} summary of the below files
which are on the topic of {{topic}}. The summary should be about {{ len }} sentences long.

{% for f in file -%}
File {{ f.filename }}:
{{ f.contents }}


{%- endfor %}
'''

[model]
# These model options can also be defined in a config file to apply to the whole directory of templates.
model = "gpt-3.5-turbo"
temperature = 0.7
# Also supports top_p, frequency_penalty, presence_penalty, stop, and max_tokens
# And format = "json"

[options]
len = { type = "int", description = "The length of the summary", default = 4 }
topic = { type = "string", description = "The topic of the summary" }
style = { type = "string", default = "concise" }
file = { type = "file", array = true, description = "The files to summarize" }
```

Then to run it:

```
> promptbox run summarize --topic software --file README.md
The README.md file provides an overview of the PromptBox utility, which is used for maintaining libraries of
LLM prompt templates that can be filled in and submitted from the command line. It explains that template files
are built in TOML and can use Liquid templating. The file also includes an example template for summarizing files
on a specific topic, with options for length, formality, and the files to summarize. Additionally, it mentions the
presence of configuration files that can set default model options and inherit settings from parent directories.

> promptbox run summarize --topic software --file README.md --style excited 
Introducing PromptBox, a powerful utility for maintaining libraries of LLM prompt templates! With PromptBox, you can
easily fill in and submit prompt templates from the command line. These template files, built in TOML, can utilize
Liquid templating and reference templates in other files. They also define command-line arguments, including references
to files. The excitement doesn't stop there! PromptBox even supports configuration files, allowing you to set default
model options and inherit settings from parent directories. Get ready to revolutionize your software experience
with PromptBox!
```

## Additional Input

Promptbox can take additional input from extra command-line arguments or have it piped in from another command.

`cat "transcript.txt" | pb run summarize "Here is the transcript:"`

By default, this content is appended to the end of the prompt, but the template can reference it as `{{extra}}`
to have it placed elsewhere in the prompt, as in this example. 
```liquid
Below is a transcript of a video named "{{title}}":

{{extra}}

Create a detailed outline of the above transcript.
```
This can be help when using this mode with models that work best when
their instructions are at end of the prompt.

## Model Choice

PromptBox supports a few model hosts, and uses a very simple logic to choose the host:

- Any model name starting with "gpt-3.5" or "gpt-4" will result in a call to the OpenAI API.
- The value "lm-studio" will result in a call to LM Studio. LM Studio's API currently does not support selecting a
    model, so you will need to switch it yourself in the GUI.
- Any other model name indicates a model from Ollama.

Models can use aliases as well. In either the template or a configuration file, you can add an `model.alias` section.


```toml
[model.alias]
phind = "phind-codellama:34b-v2-q5_K_M"
deepseek = "deepseek-coder:7b"
```

These model aliases can then be used in place of the actual model name.

## Context Length Management

When your prompts and their input start to get large, there are a few options to manage the context length.

```toml
[model.context]
# Override the context length limit from the model. Usually you can omit this unless you want to
# artificially decrease the context length to save time, money, etc.
limit = 384

# Make sure the context has enough room for this many tokens of output.
# This defaults to 256 if not otherwise specified.
# The prompt will contain roughly `limit - reserve_output` tokens.
reserve_output = 256

# When trimming context, should it keep the "start" or the "end"
keep = "start"
# keep = "end"

# The names of arguments to trim context from. If omitted, the entire prompt is trimmed to fit.
trim_args = ["extra", "files"]

# When trimming array arguments, whether to preserve the first arguments,
# the last arguments, or try to trim equally.
array_priority = "first"
# array_priority = "last"
# array_priority = "equal"
```

Right now, the Llama 2 tokenizer is used regardless of the model chosen. This won't give exact results for
every model, but will be close enough for most cases.

# Configuration Files

Each directory of templates contains a configuration file, which can set default model options. Configuration files are read
from the current directory up through the parent directories. 

In each directory searched, PromptBox will look for a configuration file in that directory and in a
`promptbox` subdirectory.

The global configuration directory such as `.config/promptbox/promptbox.toml` is read as well.

A configuration file inherits settings from the configuration files in its parent directories as well, for those options that
it does not set itself. All settings in a configuration file are optional.

```toml
# By default the templates are in the same directory as the configuration file, but this can be overridden
# by setting the templates option
templates = ["template_dir"]

# This can be set to true to tell PromptBox to stop looking in parent directories for
# configurations and templates.
top_level = false

# Set this to false to tell PromptBox to not read the global configuration file.
use_global_config = true

[model]
# Set a default model. All the other options from the template's `model` section can be used here.
model = "gpt-3.5-turbo"
```

