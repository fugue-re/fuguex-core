<p align="center">
  <img alt="Fugue logo" src="https://raw.githubusercontent.com/fugue-re/fugue-core/master/data/fugue-logo-border-t.png" width="20%">
</p>



[![DOI](https://zenodo.org/badge/386729787.svg)](https://zenodo.org/badge/latestdoi/386729787)


# Fugue Binary Analysis Framework

Fugue is a binary analysis framework in the spirit of [B2R2] and [BAP], with
a focus on providing reusable components to rapidly prototype new binary
analysis tools and techniques.

This collection of crates, i.e., `fuguex-core` can be used to build
custom interpreters. The `fuguex-concrete` crate provides a basic interpreter
based on Micro execution [microx], that can be customised with user-defined
hooks, intrinsic handlers, and execution strategies.

[BAP]: https://github.com/BinaryAnalysisPlatform/bap/
[B2R2]: https://github.com/B2R2-org/B2R2
[microx]: https://patricegodefroid.github.io/public_psfiles/icse2014.pdf
