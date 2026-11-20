#!/usr/bin/env node
'use strict';

// Fail closed even when a tag or version argument is supplied.
console.error('agyline package preparation is disabled.');
console.error('No fork npm namespace or release repository has been approved.');
console.error('No packages were generated; upstream scopes must not be reused. Build this fork from source instead.');
process.exitCode = 1;
