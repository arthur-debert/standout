# Fast Paced intro to your First Outstanding Based Command

This is a terse and direct how to for more experienced developers or at least the ones in a hurry. 
It skimps rationale , design and other useful bits you can read from the (longer form version)[full-tutorial.md]

##  Prerequisites

A cli app, that uses clap for arg parsing.
The command to be managed by oustanding having split it's logic / processing from the output formatting, that is: 
One function for getting the parsed cli args, does the business logic and  returns a Serializable struct:
<insert example here </insert>
Another function that takes the serializable struct and formats it for output :
<insert> example here </insert>

For the sake of this guide, let's call the command "shout" , and the logic part being called shout, and the other render_shout.

## Making it oustanding

### 1. File structure and files to be filed

```text
src/
├── handlers.rs      # where shout it
├── templates/
    ├── shout.jinja # the template to render shout
├── styles/
    ├── default.css # the default style for the command
``` 

### 2. Create teh Oustanding App

    Using the app builder pattern:
```rust
    let app = App::builder()
    .templates(embed_templates!("src/templates"))   // Embeds all .jinja/.j2 files
    .styles(embed_styles!("src/styles"))       // Load stylesheets
    .default_theme("default")                  // Use styles/default.css or default.yaml
    .commands(Commands::dispatch_config())          // Register handlers from derive macro
    .build()?;
```

### 3. Connect your logic to a command name and template

```rust
    #[dispatch(handlers = handlers)]
    pub enum Commands {
          ...
          Shout,
    }
```

This will by convention, link to a command and template file of the same name, which is changeable on the macro itself.

### 4. Write your styles

In the 