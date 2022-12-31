<div id="top"></div>

<p align="center">
<a href="https://github.com/kurtbuilds/req/graphs/contributors">
    <img src="https://img.shields.io/github/contributors/kurtbuilds/req.svg?style=flat-square" alt="GitHub Contributors" />
</a>
<a href="https://github.com/kurtbuilds/req/stargazers">
    <img src="https://img.shields.io/github/stars/kurtbuilds/req.svg?style=flat-square" alt="Stars" />
</a>
<a href="https://github.com/kurtbuilds/req/actions">
    <img src="https://img.shields.io/github/actions/workflow/status/kurtbuilds/req/test.yaml?style=flat-square" alt="Build Status" />
</a>
<a href="https://crates.io/crates/req">
    <img src="https://img.shields.io/crates/d/req?style=flat-square" alt="Downloads" />
</a>
<a href="https://crates.io/crates/req">
    <img src="https://img.shields.io/crates/v/req?style=flat-square" alt="Crates.io" />
</a>

</p>

# `req` - HTTP Client

`req` is a command line HTTP client, like `curl`, `httpie` and many others. Why another one? This is probably the
most succinct, while still intuitive, client you'll find. Let's see if you agree:

## Examples

```bash
# Get localhost:5000/ . We assume localhost if the hostname is a bare port.
req :5000
```

Let's send some JSON.

```bash
req :5000/auth/login --json email=test@example.com password=123
# --json sets `content-type`, sets `accept`, sets method to POST, and interprets the rest of the arguments
# as key-value pairs in JSON. And it pretty-prints and colorizes the JSON response.
# All of that behavior is defaulted, but can be overridden.
```
You can also use `--form` which behaves similarly.

Here's a GET request:

```bash
# Because its a GET request, we know additional arguments are query params. These arguments get URL encoded.
req :5000/search q='this is a multi-word query string'
```

Need authentication headers? These all work:

```bash
req --bearer <token>
req -u <user>:<pass>  # --user also works
req --token <token>
```

We keep the `-O` flag from `curl` for saving files.


# Installation

```bash
cargo install --git https://github.com/kurtbuilds/req
```

# Contributions

Need a feature or have a bug report? Open an issue or a PR.