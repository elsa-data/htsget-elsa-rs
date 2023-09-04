# htsget-elsa-rs
A htsget-rs lambda function for interacting with Elsa

[![MIT licensed][mit-badge]][mit-url]
[![Build Status][actions-badge]][actions-url]

[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: https://github.com/umccr/htsget-rs/blob/main/LICENSE
[actions-badge]: https://github.com/umccr/htsget-rs/actions/workflows/action.yml/badge.svg
[actions-url]: https://github.com/umccr/htsget-rs/actions?query=workflow%3Atests+branch%3Amain

This crate is deployed as a lambda function that wraps htsget-rs and allows it to interact with Elsa.

The lambda function first attempts to resolve the query using the manifest files from Elsa where the first path segment
in the query matches the release key in the manifest file. For example, the following matches the release key and file name:

```
GET https://<htsget_domain>/reads/<release_key>/<file>?format=BAM&referenceName=1&start=0&end=1000
```

This function supports all the regular htsget-rs configuration options, and if a query fails to match a release key, it
will fall back to the resolvers defined in the config. This crate also includes the following options to configure the
Elsa endpoint and cache location:

| Option                    | Description                                                                                            | Type          | Default             | Example                     |
|---------------------------|--------------------------------------------------------------------------------------------------------|---------------|---------------------|-----------------------------|
| `elsa_endpoint_authority` | The URL authority of the Elsa endpoint.                                                                | URL Authority | Not specified, required. | `'elsa-data.dev.umccr.org'` | 
| `cache_location`          | The name of the bucket where resolvers are cached. If this is not specified, no caching is performed.  | String        | Not specified.      | `'cache_bucket'`            |

To deploy this function, see the [deploy][deploy] folder.

[deploy]: ../deploy

## License

This project is licensed under the [MIT license][license].

[license]: LICENSE