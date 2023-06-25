use std::collections::HashSet;
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};

use crate::elsa_endpoint::ENDPOINT_PATH;
use aws_sdk_s3::Client;
use htsget_config::resolver::ReferenceNames::List;
use htsget_config::resolver::Resolver;
use htsget_config::storage;
use htsget_config::types::{Format, Interval};
use htsget_test::aws_mocks::with_s3_test_server_tmp;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate, Times};

pub fn example_elsa_manifest() -> String {
    r#"
        {
            "id": "R004",
            "reads": {
                "30F9F3FED8F711ED8C35DBEF59E9F537": {
                    "url": "s3://umccr-10g-data-dev/HG00097/HG00097.bam",
                    "restrictions": [
                        {
                            "chromosome": 1,
                            "start": 1,
                            "end": 10
                        }
                    ]
                },
                "30F9FFD4D8F711ED8C353BBCB8861211": {
                    "url": "s3://umccr-10g-data-dev/HG00096/HG00096.bam",
                    "restrictions": [
                        {
                            "chromosome": 2,
                            "end": 10
                        }
                    ]
                }
            },
            "variants": {
                "30F9F3FED8F711ED8C35DBEF59E9F537": {
                    "url": "s3://umccr-10g-data-dev/HG00097/HG00097.hard-filtered.vcf.gz",
                    "restrictions": [
                        {
                            "chromosome": 3,
                            "start": 10
                        }
                    ],
                    "variantSampleId": ""
                },
                "30F9FFD4D8F711ED8C353BBCB8861211": {
                    "url": "s3://umccr-10g-data-dev/HG00096/HG00096.hard-filtered.vcf.gz",
                    "restrictions": [
                        {
                            "chromosome": 4
                        }
                    ],
                    "variantSampleId": ""
                }
            },
            "cases": [
                {
                    "ids": {
                        "": "SINGLETONCHARLES"
                    },
                    "patients": [
                        {
                            "ids": {
                                "": "CHARLES"
                            },
                            "specimens": [
                                {
                                    "htsgetId": "30F9FFD4D8F711ED8C353BBCB8861211",
                                    "ids": {
                                        "": "HG00096"
                                    }
                                }
                            ]
                        }
                    ]
                },
                {
                    "ids": {
                        "": "SINGLETONMARY"
                    },
                    "patients": [
                        {
                            "ids": {
                                "": "MARY"
                            },
                            "specimens": [
                                {
                                    "htsgetId": "30F9F3FED8F711ED8C35DBEF59E9F537",
                                    "ids": {
                                        "": "HG00097"
                                    }
                                }
                            ]
                        }
                    ]
                }
            ]
        }
        "#
    .to_string()
}

pub(crate) fn example_elsa_response() -> String {
    r#"
        {
            "location": {
                "bucket": "elsa-data-tmp",
                "key": "htsget-manifests/R004"
            },
            "maxAge": 86400
        }
        "#
    .to_string()
}

pub fn write_example_manifest(manifest_path: &Path) {
    fs::create_dir_all(manifest_path).unwrap();
    fs::write(manifest_path.join("R004"), example_elsa_manifest()).unwrap();
}

pub async fn with_test_mocks<T, F, Fut>(test: F, expect_times: T)
where
    T: Into<Times>,
    F: FnOnce(String, Client, reqwest::Client, PathBuf) -> Fut,
    Fut: Future<Output = ()>,
{
    with_s3_test_server_tmp(|client, server_base_path| async move {
        let mock_server = MockServer::start().await;

        let base_path = server_base_path.clone();
        Mock::given(method("GET"))
            .and(path(format!("{ENDPOINT_PATH}/R004")))
            .and(query_param("type", "S3"))
            .respond_with(move |_: &Request| {
                let manifest_path = base_path.join("elsa-data-tmp/htsget-manifests");

                write_example_manifest(&manifest_path);

                ResponseTemplate::new(200).set_body_string(example_elsa_response())
            })
            .expect(expect_times)
            .mount(&mock_server)
            .await;

        test(
            mock_server.address().to_string(),
            client,
            reqwest::Client::builder().build().unwrap(),
            server_base_path,
        )
        .await;
    })
    .await;
}

pub fn is_resolver_from_parts(resolver: &Resolver) -> bool {
    resolver.regex().to_string() == "^R004/30F9F3FED8F711ED8C35DBEF59E9F537$"
        && resolver.substitution_string() == "HG00097/HG00097"
        && matches!(resolver.storage(), storage::Storage::S3 { s3_storage } if s3_storage.bucket() == "umccr-10g-data-dev")
        && resolver.allow_formats() == [Format::Bam]
        && resolver.allow_reference_names() == &List(HashSet::from_iter(vec!["1".to_string()]))
        && resolver.allow_interval() == Interval::new(Some(1), Some(10))
}

pub fn is_manifest_resolvers(resolvers: Vec<Resolver>) -> bool {
    resolvers.iter().any(|resolver| {
        resolver.regex().to_string() == "^R004/30F9FFD4D8F711ED8C353BBCB8861211$" &&
            resolver.substitution_string() == "HG00096/HG00096" &&
            matches!(resolver.storage(), storage::Storage::S3 { s3_storage } if s3_storage.bucket() == "umccr-10g-data-dev") &&
            resolver.allow_formats() == [Format::Bam]
            && resolver.allow_reference_names() == &List(HashSet::from_iter(vec!["2".to_string()]))
            && resolver.allow_interval() == Interval::new(None, Some(10))
    }) &&
    resolvers.iter().any(is_resolver_from_parts) &&
    resolvers.iter().any(|resolver| {
        resolver.regex().to_string() == "^R004/30F9FFD4D8F711ED8C353BBCB8861211$" &&
            resolver.substitution_string() == "HG00096/HG00096.hard-filtered" &&
            matches!(resolver.storage(), storage::Storage::S3 { s3_storage } if s3_storage.bucket() == "umccr-10g-data-dev") &&
            resolver.allow_formats() == [Format::Vcf]
            && resolver.allow_reference_names() == &List(HashSet::from_iter(vec!["4".to_string()]))
            && resolver.allow_interval() == Interval::new(None, None)
    }) &&
    resolvers.iter().any(|resolver| {
        resolver.regex().to_string() == "^R004/30F9F3FED8F711ED8C35DBEF59E9F537$" &&
            resolver.substitution_string() == "HG00097/HG00097.hard-filtered" &&
            matches!(resolver.storage(), storage::Storage::S3 { s3_storage } if s3_storage.bucket() == "umccr-10g-data-dev") &&
            resolver.allow_formats() == [Format::Vcf]
            && resolver.allow_reference_names() == &List(HashSet::from_iter(vec!["3".to_string()]))
            && resolver.allow_interval() == Interval::new(Some(10), None)
    })
}
