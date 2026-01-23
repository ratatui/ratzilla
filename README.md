<p align="center">
<!-- Thanks to https://github.com/dekirisu for the logo -->
<a href="https://github.com/ratatui/ratzilla"><img src="https://raw.githubusercontent.com/ratatui/ratzilla/refs/heads/main/assets/ratzilla.gif" width="500"></a>
</p>

<div align="center">

[![Repo](https://img.shields.io/badge/github-ratatui/ratzilla-3c8cba?style=flat&logo=GitHub&labelColor=1D272B&color=3c8cba&logoColor=white)](https://github.com/ratatui/ratzilla)
[![Crate](https://img.shields.io/crates/v/ratzilla?style=flat&logo=Rust&labelColor=1D272B&color=936c94&logoColor=white)](https://crates.io/crates/ratzilla)
[![Docs](https://img.shields.io/docsrs/ratzilla?style=flat&logo=Rust&labelColor=1D272B&logoColor=white)](https://docs.rs/ratzilla)

**Watch the conference talk:** [Bringing Terminal Aesthetics to the Web With Rust (and Vice Versa)](https://www.youtube.com/watch?v=iepbyYrF_YQ)

</div>

# Ratzilla

Build terminal-themed web applications with Rust and WebAssembly. Powered by [Ratatui].

## Quickstart

### Templates

Install [`cargo-generate`](https://github.com/cargo-generate/cargo-generate):

```shell
cargo install cargo-generate
```

Generate a new project:

```shell
cargo generate ratatui/ratzilla
```

And then [serve the application on your browser](#serve) ➡️

See [templates](./templates) for more information.

### Manual Setup

Add **Ratzilla** as a dependency in your `Cargo.toml`:

```sh
cargo add ratzilla
```

Here is a minimal example:

```rust no_run
use std::{cell::RefCell, io, rc::Rc};

use ratzilla::ratatui::{
    layout::Alignment,
    style::Color,
    widgets::{Block, Paragraph},
    Terminal,
};

use ratzilla::{event::KeyCode, DomBackend, WebRenderer};

fn main() -> io::Result<()> {
    let counter = Rc::new(RefCell::new(0));
    let backend = DomBackend::new()?;
    let terminal = Terminal::new(backend)?;

    terminal.on_key_event({
        let counter_cloned = counter.clone();
        move |key_event| {
            if key_event.code == KeyCode::Char(' ') {
                let mut counter = counter_cloned.borrow_mut();
                *counter += 1;
            }
        }
    });

    terminal.draw_web(move |f| {
        let counter = counter.borrow();
        f.render_widget(
            Paragraph::new(format!("Count: {counter}"))
                .alignment(Alignment::Center)
                .block(
                    Block::bordered()
                        .title("Ratzilla")
                        .title_alignment(Alignment::Center)
                        .border_style(Color::Yellow),
                ),
            f.area(),
        );
    });

    Ok(())
}
```

Add your `index.html`. During build, `trunk` will automatically inject and initialize your Rust code (compiled to
WebAssembly) as a JavaScript module.

<details>
  <summary>index.html</summary>
  
```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta
      name="viewport"
      content="width=device-width, initial-scale=1.0, user-scalable=no"
    />
    <link
      rel="stylesheet"
      href="https://cdnjs.cloudflare.com/ajax/libs/firacode/6.2.0/fira_code.min.css"
    />
    <link data-trunk rel="rust"/>
    <title>Ratzilla</title>
    <style>
      body {
        margin: 0;
        width: 100%;
        height: 100vh;
        display: flex;
        flex-direction: column;
        justify-content: center;
        align-items: center;
        align-content: center;
        background-color: #121212;
      }
      pre {
        font-family: "Fira Code", monospace;
        font-size: 16px;
        margin: 0px;
      }
    </style>
  </head>
  <body>
    <!-- (optional) subscribe to the application started event -->
    <script type="module">
      window.addEventListener("TrunkApplicationStarted", (_) => {
        // #[wasm_bindgen] functions are now bound to window.wasmBindings.*
        console.log("application initialized");
      });
    </script>
  </body>
</html>
```

</details>

And then [serve the application on your browser](#serve) ➡️

## Serve

Install [trunk] to build and serve the web application.

```sh
cargo install --locked trunk
```

Add compilation target `wasm32-unknown-unknown`:

```sh
rustup target add wasm32-unknown-unknown
```

Then serve it on your browser:

```sh
trunk serve
```

Now go to [http://localhost:8080](http://localhost:8080) and enjoy TUIs in your browser!

## Deploy

To build the WASM bundle, you can run the following command:

```sh
trunk build --release
```

Then you can serve the server from the `dist` directory.

<details>
  <summary>Example Build Script</summary>

```bash
#!/bin/bash
set -euo pipefail
export HOME=/root

# Install Rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y -t wasm32-unknown-unknown --profile minimal
source "$HOME/.cargo/env"

# Install trunk using binstall
curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
cargo binstall --targets x86_64-unknown-linux-musl -y trunk

# Build project with trunk
trunk build --release
```

</details>

### Vercel

There is a Vercel deployment template available for Ratzilla [here](https://vercel.com/templates/other/ratzilla).

## Documentation

- [API Documentation](https://docs.rs/ratzilla)
- [Backends](https://docs.rs/ratzilla/latest/ratzilla/backend/index.html)
- [Widgets](https://docs.rs/ratzilla/latest/ratzilla/widgets/index.html)

## Examples

- [Minimal](https://github.com/ratatui/ratzilla/tree/main/examples/minimal) ([Preview](https://ratatui.github.io/ratzilla/minimal))
- [Demo](https://github.com/ratatui/ratzilla/tree/main/examples/demo) ([Preview](https://ratatui.github.io/ratzilla/demo))
- [Pong](https://github.com/ratatui/ratzilla/tree/main/examples/pong) ([Preview](https://ratatui.github.io/ratzilla/pong))
- [Colors RGB](https://github.com/ratatui/ratzilla/tree/main/examples/colors_rgb) ([Preview](https://ratatui.github.io/ratzilla/colors_rgb))
- [Animations](https://github.com/ratatui/ratzilla/tree/main/examples/animations) ([Preview](https://ratatui.github.io/ratzilla/animations))
- [World Map](https://github.com/ratatui/ratzilla/tree/main/examples/world_map) ([Preview](https://ratatui.github.io/ratzilla/world_map))

## Websites built with Ratzilla

- <https://ratatui.github.io/ratzilla> - The official website of Ratzilla
- <https://terminalcollective.org> - Terminal Collective community website
- <https://www.function-type.com/tusistor> - Resistor calculator
- <http://timbeck.me> - Personal website of Tim Beck
- <https://map.apt-swarm.orca.toys> - Map of apt-swarm p2p locations
- [TachyonFX FTL](https://junkdog.github.io/tachyonfx-ftl/) - DSL editor and previewer for TachyonFX effects
- <https://emrecansuster.com> - Personal website of Emrecan Şuşter ([source](https://github.com/Tarbetu/website))
- <https://junkdog.github.io/exabind> - A tachyonfx tech demo: animated KDE keyboard shortcut viewer
- <https://rbn.dev> - Personal website of 0x01d ([source](https://github.com/0x01d/website))
- <https://gluesql.org/glues> - Glues, a Vim-inspired TUI note-taking app ([source](https://github.com/gluesql/glues))
- <https://alertangel.github.io/> - Website for AlertAngel: A device to make monitoring the Elderly a breeze. ([source](https://github.com/AlertAngel/alertangel.github.io)) (WIP)
- <https://sdr-geo-db.vercel.app/> - SDR contact logging database ([source](https://github.com/nuts-rice/sdr_geo_db))

## Acknowledgements

Thanks to [Webatui] projects for the inspiration.

Special thanks to:

- [Martin Blasko] for his huge help with the initial implementation.
- [Adrian Papari] for implementing WebGL2 backend.

Lastly, thanks to [Ratatui] for providing the core UI components.

[trunk]: https://trunkrs.dev
[Ratatui]: https://ratatui.rs
[`DomBackend`]: https://docs.rs/ratzilla/latest/ratzilla/struct.DomBackend.html
[`CanvasBackend`]: https://docs.rs/ratzilla/latest/ratzilla/struct.CanvasBackend.html
[`Hyperlink`]: https://docs.rs/ratzilla/latest/ratzilla/widgets/struct.Hyperlink.html
[Webatui]: https://github.com/TylerBloom/webatui
[Martin Blasko]: https://github.com/MartinBspheroid
[Adrian Papari]: https://github.com/junkdog
[Vercel]: https://vercel

## Contributing

Pull requests are welcome!

Consider submitting your ideas via [issues](https://github.com/ratatui/ratzilla/issues/new) first and check out the [existing issues](https://github.com/ratatui/ratzilla/issues).

## License

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=flat&logo=GitHub&labelColor=1D272B&color=3c8cba&logoColor=white)](./LICENSE-MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg?style=flat&logo=GitHub&labelColor=1D272B&color=3c8cba&logoColor=white)](./LICENSE-APACHE)

Licensed under either of [Apache License Version 2.0](./LICENSE-APACHE) or [The MIT License](./LICENSE-MIT) at your option.

🦀 ノ( º \_ º ノ) - respect crables!

## Copyright

Copyright © 2025, [Orhun Parmaksız](mailto:orhunparmaksiz@gmail.com)
