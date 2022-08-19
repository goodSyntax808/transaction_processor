# Deploying a Rust Doc Website to AWS S3

Instructions for deploying a static Rust doc site to S3.

First, let's build the Rust Documentation site.

```bash
cargo doc
cp -R target/doc/tx_processor/* target/doc
```

Finally, lets build, diff and then deploy the Rust Documentation to AWS.

```bash
cd infrastructure
npm run build
npm run diff
npm run deploy
```

Note: If you've not published to AWS via the CDK before you will likely need to run `cdk bootstrap`.

