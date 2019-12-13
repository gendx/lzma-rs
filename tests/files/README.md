# Test files #

This folder contains a collection of test files to cover different use cases of the lzma-rs library.

This README describes files that are not self-explanatory in this folder.

## range-coder-edge-cases

This is a file that causes the code and range to be equal at some point during decoding LZMA data.
Previously, this file would raise an `LZMAError("Corrupted range coding")`, although the file is a valid LZMA file.

The file was created by generating random geometry in [Blender](1) using the Array and Build
modifier on a cube.
The geometry was then exported as an FBX file that was converted into OpenCTM using the 3D service
in [Cognite Data Fusion](2).
The vertices in the resulting OpenCTM file are LZMA-compressed.
This LZMA-compressed section of the file was manually extracted and the header modified to include
the unpacked size.
The unpacked size is four times the vertex count found in the OpenCTM data.

[1]: https://blender.org
[2]: https://docs.cognite.com
