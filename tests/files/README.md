# Test files #

This folder contains a collection of test files to cover different use cases of the lzma-rs library.

This file describes files that are not self-explanatory in this folder.

## bad-random-data

This is a file that causes the code and range to be equal at some point during decoding LZMA data.
Previously, this file would raise an `LZMAError("Corrupted range coding")`, 
although the file is a valid LZMA file.
