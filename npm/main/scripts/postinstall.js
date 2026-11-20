'use strict';

// Intentionally inert: no imports, downloads, filesystem writes or host setup.
if (process.env.npm_config_loglevel !== 'silent') {
  console.log('agyline: automatic npm installation is disabled; this private helper does not install a binary or change host settings.');
  console.log('Build this fork with cargo build --release --locked, then run the built agyline install command.');
}
