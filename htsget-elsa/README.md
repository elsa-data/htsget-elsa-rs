# htsget-elsa

[![MIT licensed][mit-badge]][mit-url]
[![Build Status][actions-badge]][actions-url]

[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: https://github.com/umccr/htsget-rs/blob/main/LICENSE
[actions-badge]: https://github.com/umccr/htsget-rs/actions/workflows/action.yml/badge.svg
[actions-url]: https://github.com/umccr/htsget-rs/actions?query=workflow%3Atests+branch%3Amain

This crate is library code that allows htsget-rs to interact with Elsa.

Elsa is a genomic data sharing support software that allows access to genomic datasets using various sharing mechanisms.
A sharing mechanism that Elsa can use to define which users get access to genomic data is htsget. Elsa can restrict access based on
files, formats, genomic regions and start and end positions using htsget.

## Mechanism

Elsa and htsget-rs interact in the following way:

* Elsa has a public endpoint which returns information for a manifest file that htsget-elsa can use. 
* htsget-elsa calls this endpoint with a GET request at: `/api/manifest/htsget/<release_key>?type=S3`.
* The response is a JSON object with the following structure:
```json
{
  "location": {
    "bucket": "bucket",
    "key": "key"
  },
  "maxAge": 800
}
```
* This object can be used by htsget-elsa to fetch the manifest file from S3, which has the following structure:
```json
{
  "id": "R001",
  "reads": {
    "id": {
      "url": "s3://url/to/file.bam",
      "restrictions": [
        { "chromosome": 1, "start": 0, "end": 1000 }
      ]
    }
  },
  "variants": {
    "id": {
      "url": "s3://url/to/file.vcf.gz",
      "variantSampleId": "",
      "restrictions": [
        { "chromosome": 9, "start": 130713043, "end": 130887675 }
      ]
    }
  }
}
```
* The manifest file is used by htsget-elsa to create resolvers, which match the restrictions on the urls and are used by 
  htsget-rs when resolving queries.
* It is also cached by htsget-elsa in S3 based on the `maxAge`.

## Layout

This crate has a few components implement the htsget-rs and Elsa interaction:
* The `GetObject` trait is used by htsget-elsa to request data from cloud storage. For now this is only S3, but it could be
  extended to other cloud providers.
* The `Cache` trait is used to cache the resolvers once they have been processed. This only caches back to S3, but it could
  also use other mechanisms, including databases such as DynamoDB.
* The `ResolversFromElsa` represents the whole mechanism as described above, and is implemented by the `ElsaEndpoint` struct.

#### Feature flags

This crate has the following features:
* `test-utils`: used to enable test dependencies and functions.

## License

This project is licensed under the [MIT license][license].

[license]: LICENSE