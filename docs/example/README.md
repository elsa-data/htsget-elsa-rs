# Example of htsget-elsa process

The following shows how to demonstrate htsget-elsa returning queries based on Elsa manifests.

### Sharing with htsget enabled in Elsa
1. Enable the htsget sharing on a release with some restrictions.
2. Grab the JWT token from the data portal:
   ```bash
   export JWT_TOKEN=<JWT_TOKEN>
   ```
3. Query htsget for an allowed region:
   ```bash
   curl -H "Authorization: ${JWT_TOKEN}" "https://htsget-elsa.dev.umccr.org/reads/R001/8AE43A8E4C8111EE84492BBD28BC6E2F?referenceName=20&start=50888919&end=50931436"
   ```
   Response:
   ```json
   {
     "htsget": {
       "format": "BAM",
       "urls": [
         {
           "url": "https://<bucket>.s3.ap-southeast-2.amazonaws.com/<presigned-bam-file>",
           "headers": {
             "Range": "bytes=0-1562"
           },
           "class": "header"
         },

     ...
   
         {
           "url": "data:;base64,H4sIBAAAAAAA/wYAQkMCABsAAwAAAAAAAAAAAA==",
           "class": "body"
         }
       ]
     }
   }
   ```
4. Query htsget for a disallowed region:
   ```bash
   curl -H "Authorization: ${JWT_TOKEN}" "https://htsget-elsa.dev.umccr.org/reads/R001/8AE43A8E4C8111EE84492BBD28BC6E2F?referenceName=20&start=50931440&end=50931460"
   ```
   Response:
   ```json
   {
     "htsget": {
       "error": "NotFound",
       "message": "failed to match query with storage"
     }
   }
   ```
5. Query for a disallowed chromosome:
   ```bash
   curl -H "Authorization: ${JWT_TOKEN}" "https://htsget-elsa.dev.umccr.org/reads/R001/8AE43A8E4C8111EE84492BBD28BC6E2F?referenceName=19"
   ```
   Response:
   ```json
   {
     "htsget": {
       "error": "NotFound",
       "message": "failed to match query with storage"
     }
   }
   ```
6. Even if some of the start and end regions are allowed, all regions must be allowed for a response to be returned (e.g. start is allowed, but end is not):
   ```bash
   curl -H "Authorization: ${JWT_TOKEN}" "https://htsget-elsa.dev.umccr.org/reads/R001/8AE43A8E4C8111EE84492BBD28BC6E2F?referenceName=20&start=50888919&end=51000000"
   ```
   Response:
   ```json
   {
     "htsget": {
       "error": "NotFound",
       "message": "failed to match query with storage"
     }
   }
   ```
   
### Sharing with htsget not enabled in Elsa
1. Grab the JWT token from the data portal:
   ```bash
   export JWT_TOKEN=<JWT_TOKEN>
   ```
2. Try query htsget:
   ```bash
   curl -H "Authorization: ${JWT_TOKEN}" "https://htsget-elsa.dev.umccr.org/reads/R001/8AE43A8E4C8111EE84492BBD28BC6E2F?referenceName=20&start=50888919&end=50931436"
   ```
   Response:
   ```json
   {
     "htsget": {
       "error": "NotFound",
       "message": "failed to match query with storage"
     }
   }
   ```