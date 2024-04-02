<div align="center">
  <a href="https://github.com/uros-5/htmx-lsp2#gh-light-mode-only"><img src="assets/logo.svg#gh-light-mode-only"        width="300px" alt="HTMX-LSP logo"/></a>
  <a href="https://github.com/uros-5/htmx-lsp2#gh-dark-mode-only"><img src="assets/logo.darkmode.svg#gh-dark-mode-only" width="300px" alt="HTMX-LSP logo"/></a>
  <br>
  <a href="https://crates.io/crates/htmx-lsp2"><img alt="crates.io" src="https://img.shields.io/crates/v/htmx-lsp2.svg?style=for-the-badge&color=fdbb39&logo=rust" height="20"></a>
  <a href="https://github.com/uros-5/htmx-lsp2/actions?query=branch%3Amaster"><img alt="build status" src="https://img.shields.io/github/actions/workflow/status/uros-5/htmx-lsp2/ci.yml?branch=main&style=for-the-badge&logo=github" height="20"></a>
</div>

<h4 align="center">
     its so over
</h4>

## Installation

```console
cargo install htmx-lsp2
```

## Configuration

```json
{ 
  "lang": "rust",
  "template_ext": "jinja",
  "templates": ["./templates"],
  "js_tags": ["./frontend"],
  "backend_tags": ["./backend"]
}
```

## Supported languages

Go, Python, JavaScript, TypeScript, Rust

## When to use htmx-lsp or this lsp ?

If you are working on small hello world example web app, then you probably don't need this improved version of htmx-lsp.

For bigger projects(many templates, many backend routes, few JavaScript modules) you should try this. 

## Difference between htmx-lsp and this lsp

### Backend/frontend tags

Tags are similar to JSDoc comments. In some sense they act like documentation for htmx part of your application.
They are helpful when you arrive at some htmx project. In order to understand codebase you constantly need to browse
through files or use global search for every part of project.
With tags you can simply use goto definition feature for that attribute and _you are exactly where you need to be_. 

#### Example

```rust
fn example() {
  // some code ...

  // part of app that is 'connected' with htmx template
  // this is tag hx@tag1
  // ...
}
```

```html
<!-- some nested html -->
<a hx-get="/some_route" hx-lsp="tag1">hello world</a>
```

It is also possible to have multiple tags on one element. Some tag is in your Go function, other can be in JavaScript.
This improves _locality of behavior_, you don't need to think too much, you just quickly read and act. 

If you use editor that behind the scenes uses TreeSitter, then you can include one query in `textobjects.scm`
that can move your cursor to hx-lsp tag:

```scheme
(
  (attribute_name) @attr 
  (quoted_attribute_value
    (attribute_value) @class.inside
  ) @class.around
    (#eq? @attr "hx-lsp")
)
```

To have syntax highlighting for tags in your backend language or JavaScript use this query(it's _comment grammar_, `highlights.scm`):

```scheme
("text" @hint
 (#match? @hint "^(hx@)"))
```

And here is one for template(it's _html grammar_, `highlights.scm`):

```scheme
((attribute_name) @keyword
  (quoted_attribute_value
    (attribute_value) @function
  )
  (#eq? @keyword "hx-lsp")
)
```

#### Goto reference

There are situations where you tag is used in multiple places in one template or in many templates that are located deep in your directory tree.
To avoid messing with global search just call goto reference on tag definition and you can check each htmx-lsp instance. 

https://github.com/uros-5/htmx-lsp2/assets/59397844/786c9312-6792-4d22-b1b8-c4b00bdc58f3

If there is only one reference for tag, then you will be redirected directy to that location.

#### Goto definition

https://github.com/uros-5/htmx-lsp2/assets/59397844/dc744a59-8902-44bf-9bd0-1a1d6188d4ca

#### Goto implementation

If your editor doesn't support TreeSitter, you can use goto implementation feature for navigating between `htmx-lsp` attributes.
When you are deep in some template, searching for `htmx-lsp` attribute can be tedious, so this feature can help use navigating to our target much faster.

https://github.com/uros-5/htmx-lsp2/assets/59397844/032c32f8-2c5f-4401-8999-2792383dc49c

#### Incremental parsing for TreeSitter

One problem with htmx-lsp is that it doesn't use full power of TreeSitter, and that is incremental parsing.
Right now for every change inside of template entire file is sent and parsed by TreeSitter. And second problem is that only takes first
change that is sent by text editor.
So what if you use multiple cursors and you start applying changes for text document ? And what if client only supports
incremental synchronization for `didChange` lsp feature ?
This forked version of htmx-lsp aims to fix this issues, not just for template part, but also for backend languages.

#### VSCode plugin

It's still work in progress. Right now it's usable in debug mode.
