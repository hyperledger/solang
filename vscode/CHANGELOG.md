# Change Log

All notable changes to the "solang" extension will be documented in this file.

## [0.3.4]
- Language server code completion is fully implemented. [chioni16](https://github.com/chioni16)
- Fixed a bug in the renaming functionality. [chioni16](https://github.com/chioni16)
- Fixed common panics. [seanyoung](https://github.com/seanyoung)
- Improve overloaded function call diagnostics: From Solidity 0.6 onwards,
  when an overloaded function/event resolves to multiple candidates, this is an error.
  In earlier versions, the first result is used.
  So, use the pragma solidity version to decide whether to error or not.
  [seanyoung](https://github.com/seanyoung)
- Updated dependencies.

## [0.3.3]

- The same version of solang should be used by the language server as on the command line,
  so first look in the $PATH for solang before downloading the solang binary. As a result, the
  `forceSolangExecutable` option is no longer needed. [seanyoung](https://github.com/seanyoung)
- Go to definition, go to type definition, go to implementation is implemented. [chioni16](https://github.com/chioni16)
- Rename functionality is now implemented. [chioni16](https://github.com/chioni16)
- It is not longer necessary to save a Solidity file, in order for the language server to pick
  up changes to the file. [chioni16](https://github.com/chioni16)

## [0.3.0]

- Ensure the extension still works without a connections to the internet
- Allow solang executable to set explicity to a path using
  solang.forceSolangExecutable
- Remove unsupported targets Sawtooth and
- Updates for solang v0.1.10

## [0.2.0]

- Automatically download a newer version of solang if available
- Use the arm mac binary if running on apple silicon

## [0.1.0]

- Initial release
