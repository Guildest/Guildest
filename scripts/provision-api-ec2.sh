#!/usr/bin/env bash

set -euo pipefail

AWS_REGION="${AWS_REGION:-us-east-1}"
INSTANCE_NAME="${INSTANCE_NAME:-guildest-api}"
INSTANCE_TYPE="${INSTANCE_TYPE:-t2.micro}"
KEY_NAME="${KEY_NAME:-guildest-api-deploy}"
SECURITY_GROUP_NAME="${SECURITY_GROUP_NAME:-guildest-api-sg}"
AMI_ID="${AMI_ID:-}"
SUBNET_ID="${SUBNET_ID:-}"
SSH_CIDR="${SSH_CIDR:-0.0.0.0/0}"
IAM_INSTANCE_PROFILE="${IAM_INSTANCE_PROFILE:-}"

if [[ -z "$AMI_ID" ]]; then
  AMI_ID="$(
    aws ec2 describe-images \
      --region "$AWS_REGION" \
      --owners amazon \
      --filters "Name=name,Values=amzn2-ami-hvm-*-x86_64-gp2" "Name=state,Values=available" \
      --query 'sort_by(Images, &CreationDate)[-1].ImageId' \
      --output text
  )"
fi

if [[ -z "$SUBNET_ID" ]]; then
  SUBNET_ID="$(
    aws ec2 describe-subnets \
      --region "$AWS_REGION" \
      --filters "Name=default-for-az,Values=true" "Name=map-public-ip-on-launch,Values=true" \
      --query 'Subnets[0].SubnetId' \
      --output text
  )"
fi

VPC_ID="$(
  aws ec2 describe-subnets \
    --region "$AWS_REGION" \
    --subnet-ids "$SUBNET_ID" \
    --query 'Subnets[0].VpcId' \
    --output text
)"

if ! aws ec2 describe-key-pairs --region "$AWS_REGION" --key-names "$KEY_NAME" >/dev/null 2>&1; then
  aws ec2 create-key-pair \
    --region "$AWS_REGION" \
    --key-name "$KEY_NAME" \
    --query 'KeyMaterial' \
    --output text > "${KEY_NAME}.pem"
  chmod 600 "${KEY_NAME}.pem"
  echo "Created SSH key: ${KEY_NAME}.pem"
fi

SECURITY_GROUP_ID="$(
  aws ec2 describe-security-groups \
    --region "$AWS_REGION" \
    --filters "Name=group-name,Values=$SECURITY_GROUP_NAME" "Name=vpc-id,Values=$VPC_ID" \
    --query 'SecurityGroups[0].GroupId' \
    --output text
)"

if [[ "$SECURITY_GROUP_ID" == "None" ]]; then
  SECURITY_GROUP_ID="$(
    aws ec2 create-security-group \
      --region "$AWS_REGION" \
      --group-name "$SECURITY_GROUP_NAME" \
      --description "Guildest API HTTP, HTTPS, and SSH" \
      --vpc-id "$VPC_ID" \
      --query 'GroupId' \
      --output text
  )"

  aws ec2 authorize-security-group-ingress \
    --region "$AWS_REGION" \
    --group-id "$SECURITY_GROUP_ID" \
    --ip-permissions \
      "IpProtocol=tcp,FromPort=22,ToPort=22,IpRanges=[{CidrIp=$SSH_CIDR,Description=SSH}]" \
      "IpProtocol=tcp,FromPort=80,ToPort=80,IpRanges=[{CidrIp=0.0.0.0/0,Description=HTTP}]" \
      "IpProtocol=tcp,FromPort=443,ToPort=443,IpRanges=[{CidrIp=0.0.0.0/0,Description=HTTPS}]"
fi

USER_DATA="$(mktemp)"
cat > "$USER_DATA" <<'EOF'
#!/bin/bash
set -euxo pipefail
if command -v dnf >/dev/null 2>&1; then
  dnf update -y
  dnf install -y docker
else
  yum update -y
  yum install -y docker
fi
systemctl enable --now docker
usermod -aG docker ec2-user
mkdir -p /usr/local/lib/docker/cli-plugins
/usr/bin/curl -fsSL https://github.com/docker/compose/releases/latest/download/docker-compose-linux-x86_64 -o /usr/local/lib/docker/cli-plugins/docker-compose
chmod +x /usr/local/lib/docker/cli-plugins/docker-compose
mkdir -p /opt/guildest-api
chown ec2-user:ec2-user /opt/guildest-api
EOF

INSTANCE_ID="$(
  IAM_PROFILE_ARGS=()
  if [[ -n "$IAM_INSTANCE_PROFILE" ]]; then
    IAM_PROFILE_ARGS=(--iam-instance-profile "Name=$IAM_INSTANCE_PROFILE")
  fi

  aws ec2 run-instances \
    --region "$AWS_REGION" \
    --image-id "$AMI_ID" \
    --instance-type "$INSTANCE_TYPE" \
    --key-name "$KEY_NAME" \
    "${IAM_PROFILE_ARGS[@]}" \
    --subnet-id "$SUBNET_ID" \
    --security-group-ids "$SECURITY_GROUP_ID" \
    --associate-public-ip-address \
    --block-device-mappings 'DeviceName=/dev/xvda,Ebs={VolumeSize=16,VolumeType=gp3,DeleteOnTermination=true}' \
    --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=$INSTANCE_NAME}]" \
    --user-data "file://$USER_DATA" \
    --query 'Instances[0].InstanceId' \
    --output text
)"

rm -f "$USER_DATA"

aws ec2 wait instance-running --region "$AWS_REGION" --instance-ids "$INSTANCE_ID"

PUBLIC_IP="$(
  aws ec2 describe-instances \
    --region "$AWS_REGION" \
    --instance-ids "$INSTANCE_ID" \
    --query 'Reservations[0].Instances[0].PublicIpAddress' \
    --output text
)"

echo "INSTANCE_ID=$INSTANCE_ID"
echo "PUBLIC_IP=$PUBLIC_IP"
echo "SSH_USER=ec2-user"
echo "SSH_KEY=${KEY_NAME}.pem"
