#!/usr/bin/env node
import "source-map-support/register";
import * as cdk from "aws-cdk-lib";
import { HtsgetElsaLambdaStack } from "../lib/htsget-elsa-lambda-stack";

export const STACK_NAME = "HtsgetElsaLambdaStack";
const STACK_DESCRIPTION =
  "A stack deploying htsget-elsa-lambda with API gateway.";

const app = new cdk.App();
new HtsgetElsaLambdaStack(app, STACK_NAME, {
  stackName: STACK_NAME,
  description: STACK_DESCRIPTION,
  tags: {
    Stack: STACK_NAME,
  },
  env: {
    account: process.env.CDK_DEFAULT_ACCOUNT,
    region: process.env.CDK_DEFAULT_REGION,
  },
});
