# htsget-elsa

[![MIT licensed][mit-badge]][mit-url]
[![Build Status][actions-badge]][actions-url]

An extension of [htsget-rs][htsget-rs] for using the htsget protocol with [Elsa][Elsa] to share data.
This project contains two crates that enable sharing data defined by Elsa manifests through htsget-rs.
htsget-elsa can fetch manifests from Elsa and convert them to resolvers that are used by htsget-rs to allow selectively
querying certain files, regions, formats, start, or end positions.

[Elsa]: https://github.com/elsa-data/elsa-data
[htsget-rs]: https://github.com/umccr/htsget-rs
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: https://github.com/umccr/htsget-rs/blob/main/LICENSE
[actions-badge]: https://github.com/umccr/htsget-rs/actions/workflows/action.yml/badge.svg
[actions-url]: https://github.com/umccr/htsget-rs/actions?query=workflow%3Atests+branch%3Amain

## Usage

This project implements a Lambda function which can be deployed to AWS. See [deploy][deploy] for more information.

[deploy]: deploy

## Project Layout

There are two crates which enable htsget-rs to convert Elsa manifests to resolvers.

See the READMEs of these crates for more details:
* [htsget-elsa-lambda][htsget-elsa-lambda]: A Lambda function which enables htsget-elsa functionality.
* [htsget-elsa][htsget-elsa]: The library code which implements the htsget-rs and Elsa interaction.

[htsget-elsa-lambda]: htsget-elsa-lambda
[htsget-elsa]: htsget-elsa

## License

This project is licensed under the [MIT license][license].

[license]: LICENSE
