# PromptBox

This utility allows maintaining libraries of LLM prompt templates which can be filled in and submitted from the command
line.

# Template Files

- are built in TOML
- can use Liquid templating, and reference templates in other files
- define command-line arguments, which can include references to files

```toml
# File: summarize.pb.toml

description = "Summarize some files"

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

# Configuration Files

Each directory of templates contains a configuration file, which can set default model options. Config files are read
from the current directory up through the parent directories, and the global configuration directory such as
`.config/promptbox/promptbox.toml` is read as well.

A configuration file inherits settings from the config files in its parent directories as well, for those options that
it does not set itself.

