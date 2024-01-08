import sys

# List of AWS regions
region_map = {
    # Frankfurt
    "eu_central_1": "ami-0faab6bdbac9486fb",
    # Ireland
    "eu_west_1": "ami-0905a3c97561e0b69",
    # London
    "eu_west_2": "ami-0e5f882be1900e43b",
    # Paris
    "eu_west_3": "ami-01d21b7be69801c2f",
    # Stockholm
    "eu_north_1": "ami-0014ce3e52359afbd",
    # Milan
    "eu_south_1": "ami-056bb2662ef466553",
    # Spain
    "eu_south_2": "ami-0a9e7160cebfd8c12",
    # N. Virgina
    "us_east_1": "ami-0c7217cdde317cfec",
    # Ohio
    "us_east_2": "ami-05fb0b8c1424f266b",
    # N. Cali
    "us_west_1": "ami-0ce2cb35386fc22e9",
    # Oregon
    "us_west_2": "ami-008fe2fc65df48dac",
    # Capetown
    "af_south_1": "ami-0e878fcddf2937686",
    # Hong Kong
    "ap_east_1": "ami-0d96ec8a788679eb2",
    # Tokio 
    "ap_northeast_1": "ami-07c589821f2b353aa",
    # Seoul
    "ap_northeast_2": "ami-0f3a440bbcff3d043",
    # Osaka
    "ap_northeast_3": "ami-05ff0b3a7128cd6f8",
    # Mumbai
    "ap_south_1": "ami-03f4878755434977f",
    # Hydrabad
    "ap_south_2": "ami-0bbc2f7f6287d5ca6",
    # Singapore
    "ap_southeast_1": "ami-0fa377108253bf620",
    # Sydney
    "ap_southeast_2": "ami-04f5097681773b989",
    # Jakarta
    "ap_southeast_3": "ami-02157887724ade8ba",
    # Bahrain
    "me_south_1": "ami-0ce1025465c85da8d",
    # UAE
    "me_central_1": "ami-0b98fa71853d8d270",
    # Canada
    "ca_central_1": "ami-0a2e7efb4257c0907",
    # Sao Paolo
    "sa_east_1": "ami-0fb4cf3a99aa89f72"
}


template = """
resource "aws_security_group" "sg-REGION" {
  provider        = aws.REGION
  name        = "allow_all"

  ingress {
    from_port        = 0
    to_port          = 0
    protocol         = "-1"
    cidr_blocks      = ["0.0.0.0/0"]
  }

  egress {
    from_port        = 0
    to_port          = 0
    protocol         = "-1"
    cidr_blocks      = ["0.0.0.0/0"]
  }

  tags = {
    Name = "experiment"
  }
}

resource "aws_key_pair" "key-REGION" {
  provider        = aws.REGION
  key_name   = "my-terraform-key-REGION"
  public_key = tls_private_key.experiment.public_key_openssh
}

resource "aws_instance" "instance-REGION" {
  provider        = aws.REGION
  ami             = "AMI"
  instance_type   = "t3.micro"
  key_name = aws_key_pair.key-REGION.key_name
  vpc_security_group_ids = [aws_security_group.sg-REGION.id]

  tags = {
    Name = "experiment"
  }
  user_data = <<EOF
#!/bin/bash

# Download the binary from the pre-signed S3 URL
curl -o /tmp/binary "${var.runner_url}"

# Make binary executable
chmod +x /tmp/binary
EOF
}


resource "null_resource" "prov-REGION" {
  depends_on = DEPENDS_ON

  provisioner "remote-exec" {
    connection {
      host        = aws_instance.instance-REGION.public_ip
      user        = "ubuntu"
      private_key = tls_private_key.experiment.private_key_pem
    }

    inline = [
      "sleep 30",
      "/tmp/binary --id ID --message-size MESSAGE_SIZE --message-rate MESSAGE_RATE --port 4100 --peers-addrs PEERS_ADDRS"
    ]
  }
}
"""

merged = ""




num_regions = sys.argv[1]
message_size = sys.argv[2]
message_rate = sys.argv[3]

id = 0
for region, ami in sorted(region_map.items()):
  if id + 1 > int(num_regions):
    break
  depends_on = [f"aws_instance.instance-{region}" for region in sorted(region_map)]
  depends_on = f"[{', '.join(depends_on)}]"
  peers_addrs = [f"${{aws_instance.instance-{r}.public_ip}}:4100" for r in sorted(region_map) if r != region]
  peers_addrs = ' '.join(peers_addrs)
  merged += template.replace("REGION", region).replace("AMI",ami).replace("DEPENDS_ON",depends_on).replace("PEERS_ADDRS", peers_addrs).replace("ID", str(id)).replace("MESSAGE_SIZE", message_size).replace("MESSAGE_RATE", message_rate)
  id += 1 
 
with open("providers.txt") as f:
    data = f.read()
     
with open('main.tf', 'w') as f:
    # Define the data to be written
    f.write(data+merged)
