Introduction
===========

`Weblite` is an implementation of the *[ECHONET Lite Web API Guidelines](https://echonet.jp/web_api/#guideline)*. `Weblite` is implemented as a Web Service and provides a web API to control ECHONET Lite devices.

`Weblite` is written in Rust using the `actix-web` framework. 

The latest source code is always available at the [author's github repository](https://github.com/s-marios/weblite).

Compiling the program
---------------------
Assuming that you have `git` and the Rust toolchain installed on your computer you can compile with the following commands:

```
git clone https://github.com/s-marios/weblite.git
cd weblite
cargo build
```

Backend
-------
The backend for `weblite` is [`echology`](https://github.com/s-marios/echology). Compile it from source or grab one of the binary distributions available on the release page. Make sure that an `echology` backend is deployed in your home/testing environment before starting up `weblite`. 

Configuration
-------------
Currently, the main application expects a configuration file named `config.json` to reside at your current working directory, which usually is the top directory of this source distribution.

A sample config file can be found in the top directory, named `config.json.sample` with the following contents:

```
{ 
  "backend" : "127.0.0.1:3361",
  "dd_dir" : "./dd/",
  "superclass_dd" : "./dd/commonItems_v120.json",
  "ai_file" : "./ai/ai.json"
}
```


- `backend` is the `echology` instance that is running as the backend
- `dd_dir` is the *device description* directory
- `superclass_dd` is the device description file that describes common itmes (usually commonItmes_xxx.json)
- `ai_file` is the file that contains *additional information* used to generate converters for properties


Copy `config.json.sample` to `config.json` and adjust it to your needs.


Running the Program
-------------------
Make sure that `config.json` is present in the top directory.


If you have compiled from source you can use `cargo` in the top directory:
```
cargo run
```

If you are using a binary distribution, from the top directory do:
```
target/debug/weblite
```

About Device Descriptions
-------------------------

Although device descriptions are available [here (version 1.2.0)](https://echonet.jp/wp/wp-content/uploads/pdf/General/Download/web_API/Web_API_device_descriptions_v1.2.0.zip), you are strongly advised to use the curated device descriptions that are distributed in root folder `/dd`. These may contain additional experimental definitions, as well as specific versions of each device description that may contain further fixes and modifications if deemed necessary.

Checking Device Descriptions with `ddcheck`
--------------------------------------------

`ddcheck` is a tool that checks device descriptions for validity in terms of syntax. From the top directory use

```
cargo run --bin ddcheck [FILE_TO_CHECK] ...
```

or,
```
target/debug/ddcheck [FILE_TO_CHECK] ...
```

For example, to check the curated device descriptions present in `./dd` do:
```
cargo run --bin ddcheck ./dd/*.json
```

..some of them will report errors :).

Finally, it may very well be the case that *not all the properties present in a device description are usable*. This is due to the fact that many properties (especially object properties) require *additional information* so that a usable converter to/from binary may be generated. Automatic generation of converters for properties is still a work in progress.
