import {
  Stack,
  StackProps,
  RemovalPolicy,
  CfnOutput,
  Duration,
} from "aws-cdk-lib";
import { Construct } from "constructs";
import { Distribution, OriginAccessIdentity } from "aws-cdk-lib/aws-cloudfront";
import { S3Origin } from "aws-cdk-lib/aws-cloudfront-origins";
import {
  Bucket,
  BucketAccessControl,
  BlockPublicAccess,
  StorageClass,
} from "aws-cdk-lib/aws-s3";
import { BucketDeployment, Source } from "aws-cdk-lib/aws-s3-deployment";
import * as path from "path";

export class InfrastructureStack extends Stack {
  constructor(scope: Construct, id: string, props?: StackProps) {
    super(scope, id, props);

    // Define your Domain nmae here
    // docs.sunnyengland.com

    // Static Site Bucket
    const siteBucket = new Bucket(this, `TransactionDocSite`, {
      bucketName: `transactionsite`,
      publicReadAccess: false,
      accessControl: BucketAccessControl.PRIVATE,
      blockPublicAccess: BlockPublicAccess.BLOCK_ALL,
      removalPolicy: RemovalPolicy.DESTROY,
      enforceSSL: true,
      autoDeleteObjects: true, // When in Prod you prob want False
      lifecycleRules: [
        {
          enabled: true,
          abortIncompleteMultipartUploadAfter: Duration.days(1),
        },
        {
          enabled: true,
          expiration: Duration.days(366),
        },
        {
          transitions: [
            {
              // Cost effective decision making!
              storageClass: StorageClass.INTELLIGENT_TIERING,
              transitionAfter: Duration.days(10),
            },
          ],
        },
      ],
    });

    // Use CloudFront Distribution to serve the files from the s3 bucket
    const originAccessIdentity = new OriginAccessIdentity(
      this,
      "OriginAccessIdentity"
    );
    siteBucket.grantRead(originAccessIdentity);
    const distribution = new Distribution(this, "Distribution", {
      defaultRootObject: "index.html",
      defaultBehavior: {
        origin: new S3Origin(siteBucket, { originAccessIdentity }),
      },
    });

    // Upload our Rust Docs
    new BucketDeployment(this, "BucketDeployment", {
      destinationBucket: siteBucket,
      sources: [Source.asset(path.resolve(__dirname, "../../target/doc"))],
    });

    new CfnOutput(this, "siteBucket", { value: siteBucket.bucketName });
    new CfnOutput(this, "DistributionDomainName", {
      value: distribution.distributionDomainName,
    });
  }
}
