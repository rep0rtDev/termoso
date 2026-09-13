import {
  errorMessage,
  isDesktopError,
  type AwsService,
  type CloudAddressType,
  type CloudConfig,
  type CloudPreview,
  type CloudProvider,
} from "@/ipc/types";

export const CLOUD_PROVIDERS: { id: CloudProvider; name: string; short: string }[] = [
  { id: "aws", name: "Amazon AWS", short: "AWS" },
  { id: "digital_ocean", name: "DigitalOcean", short: "DigitalOcean" },
  { id: "azure", name: "Microsoft Azure", short: "Azure" },
];

/** Commercial EC2 regions; anything else can be typed in. */
export const AWS_REGIONS = [
  "us-east-1",
  "us-east-2",
  "us-west-1",
  "us-west-2",
  "af-south-1",
  "ap-east-1",
  "ap-south-1",
  "ap-south-2",
  "ap-southeast-1",
  "ap-southeast-2",
  "ap-southeast-3",
  "ap-southeast-4",
  "ap-northeast-1",
  "ap-northeast-2",
  "ap-northeast-3",
  "ca-central-1",
  "ca-west-1",
  "eu-central-1",
  "eu-central-2",
  "eu-west-1",
  "eu-west-2",
  "eu-west-3",
  "eu-north-1",
  "eu-south-1",
  "eu-south-2",
  "il-central-1",
  "me-south-1",
  "me-central-1",
  "sa-east-1",
];

export const PRIVACY_NOTE =
  "Keys and tokens are used for this request only, inside the app, and are not saved, synced or logged. Termoso never sends them anywhere but the provider's own API.";

export interface Draft {
  aws: {
    region: string;
    accessKeyId: string;
    secretAccessKey: string;
    service: AwsService;
    addressType: CloudAddressType;
  };
  digitalOcean: { token: string };
  azure: { tenantId: string; clientId: string; clientSecret: string };
}

export const emptyDraft = (): Draft => ({
  aws: {
    region: "us-east-1",
    accessKeyId: "",
    secretAccessKey: "",
    service: "ec2",
    addressType: "public",
  },
  digitalOcean: { token: "" },
  azure: { tenantId: "", clientId: "", clientSecret: "" },
});

export function toConfig(provider: CloudProvider, d: Draft): CloudConfig | null {
  const t = (s: string) => s.trim();
  switch (provider) {
    case "aws":
      if (!t(d.aws.region) || !t(d.aws.accessKeyId) || !t(d.aws.secretAccessKey)) return null;
      return {
        provider: "aws",
        region: t(d.aws.region),
        access_key_id: t(d.aws.accessKeyId),
        secret_access_key: t(d.aws.secretAccessKey),
        service: d.aws.service,
        address_type: d.aws.addressType,
      };
    case "digital_ocean":
      if (!t(d.digitalOcean.token)) return null;
      return { provider: "digital_ocean", token: t(d.digitalOcean.token) };
    case "azure":
      if (!t(d.azure.tenantId) || !t(d.azure.clientId) || !t(d.azure.clientSecret)) return null;
      return {
        provider: "azure",
        tenant_id: t(d.azure.tenantId),
        client_id: t(d.azure.clientId),
        client_secret: t(d.azure.clientSecret),
      };
  }
}

/** Human wording for the typed provider errors coming from Rust. */
export function cloudErrorMessage(e: unknown, providerName: string): string {
  if (isDesktopError(e)) {
    switch (e.kind) {
      case "cloud_invalid_credentials":
        return `${providerName} was not able to validate the provided access credentials.`;
      case "cloud_forbidden":
        return `${providerName} rejected the request: the credentials lack permission to list machines. ${e.message}`;
      case "rate_limited":
        return `${providerName} is rate limiting requests. Wait a moment and try again.`;
      case "cloud_unavailable":
        return `${providerName} could not be reached. Check your connection and try again. ${e.message}`;
      case "cloud_malformed":
        return `${providerName} returned an unexpected response. ${e.message}`;
    }
  }
  return errorMessage(e);
}

export const importable = (p: CloudPreview) =>
  p.instances.flatMap((i, idx) => (i.action === "no_address" ? [] : [idx]));
