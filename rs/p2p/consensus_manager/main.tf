provider "aws" {
  alias  = "us_east_1"
  region = "us-east-1"
}

provider "aws" {
  alias  = "us_east_2"
  region = "us-east-2"
}

provider "aws" {
  alias  = "us_west_1"
  region = "us-west-1"
}

provider "aws" {
  alias  = "us_west_2"
  region = "us-west-2"
}

provider "aws" {
  alias  = "af_south_1"
  region = "af-south-1"
}

provider "aws" {
  alias  = "ap_east_1"
  region = "ap-east-1"
}

provider "aws" {
  alias  = "ap_south_1"
  region = "ap-south-1"
}

provider "aws" {
  alias  = "ap_northeast_3"
  region = "ap-northeast-3"
}

provider "aws" {
  alias  = "ap_northeast_2"
  region = "ap-northeast-2"
}

provider "aws" {
  alias  = "ap_southeast_1"
  region = "ap-southeast-1"
}

provider "aws" {
  alias  = "ap_southeast_2"
  region = "ap-southeast-2"
}

provider "aws" {
  alias  = "ap_northeast_1"
  region = "ap-northeast-1"
}

provider "aws" {
  alias  = "ca_central_1"
  region = "ca-central-1"
}

provider "aws" {
  alias  = "eu_central_1"
  region = "eu-central-1"
}

provider "aws" {
  alias  = "eu_west_1"
  region = "eu-west-1"
}

provider "aws" {
  alias  = "eu_west_2"
  region = "eu-west-2"
}

provider "aws" {
  alias  = "eu_south_1"
  region = "eu-south-1"
}

provider "aws" {
  alias  = "eu_west_3"
  region = "eu-west-3"
}

provider "aws" {
  alias  = "eu_north_1"
  region = "eu-north-1"
}

provider "aws" {
  alias  = "me_south_1"
  region = "me-south-1"
}

provider "aws"{
  alias  = "sa_east_1"
  region = "sa-east-1"
}

provider "aws" {
  alias  = "cn_north_1"
  region = "cn-north-1"
}

provider "aws" {
  alias  = "cn_northwest_1"
  region = "cn-northwest-1"
}


variable "runner_url" {
  type        = string
  description = "presigned s3 runner url"
  default     = "https://conesnsus-binary.s3.eu-central-1.amazonaws.com/consensus_manager_runner?response-content-disposition=inline&X-Amz-Security-Token=IQoJb3JpZ2luX2VjEPL%2F%2F%2F%2F%2F%2F%2F%2F%2F%2FwEaDGV1LWNlbnRyYWwtMSJHMEUCIFOoc6nZlZk2A%2FpdEqrXPCl0UsTJgyurXnH5EZvrBFysAiEAmlZUyty27Vo42i2LDbTKIu3KlZexDziHhEpddO7GmFMq7QIIi%2F%2F%2F%2F%2F%2F%2F%2F%2F%2F%2FARAEGgw1MTcxMzEzNDE4NjEiDBNAcxTjLraNtEcwyirBAtOkyJL55E4qIdXG7E9GZrW4P2Nt40lbpZ%2FzermVmk18fzxZpzfDgQQddRrqJw%2FertAF2GI%2F0DTHJxg%2BwfrAX85QMI2YFdKVjwwrn8CS1H2TD1zkWpA7GX0hZSCU4qlfjPclHGqrR83RQrVM7mEYwFKQo71CTCX3gbwkiEAv0rmABGP0eKm7%2F2syFe%2FRheopHOSRxePUW%2FXAhLwQglRk%2FT5JsdyohE0LcMOOLtMEizl6X9A275N4fPzlb0VEyN%2FOB4a6qY5i2hEOmpMuucqZy8Vhc6WK7SWt8PvFrYaSiepbz214ydup5HqcGkb08yFzlZmGoFFaTynTurJohAA8WLEzbZIo%2FxGnRhqebqkZYvRoQHTgdYiDA0ZMgpnDCRRvSjOE%2FTq0iNI9JiQK4Lih04L%2FlHZicCuj6pACiTNT3zPACjCY8N6sBjqzAl2Y1X4RWENrFiKbH%2F8S0LbLeTyCLTAlcW7mJm%2FND%2BWK3tgv0UeNarl%2BLlcsHOVHo7aKiE8YS2zZ5d%2Bh9o4vftt9QYYrLcG5TfCtfMBwc%2FdMob9F9EhiBE%2FMDzSpqIFJSQPhqDSwIr92lTQchlJ77jl2q4hnw9bxjRA25DGZ9ZWupsECYit4s10wXJO%2BC4Cz2lbbBnCq06zW6Ig%2FbnnuMpCZHtWXFOVNBrFEsB9Nh17ac15Jhl94aQ%2F1CJQ%2FZblO5lnLro2XqpqWktZUGl1%2BM4oOZAkjuZAivpYI9dnjlsqnO%2Bp0c5ZTHrEuHsVIlhbynF54%2F2eTXQSZJrrLYJF%2FlBmzsyZxKdiB4rjJPxSGormhmipLCmcWFcGFAEmmghTQU5Sld6Rqqiza1To6J9jTYNLWTtQ%3D&X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Date=20240105T100108Z&X-Amz-SignedHeaders=host&X-Amz-Expires=7200&X-Amz-Credential=ASIAXQZ3OCASRWYLIE42%2F20240105%2Feu-central-1%2Fs3%2Faws4_request&X-Amz-Signature=a593ea038a7cc6130176105013555cdb919b41ac646e5b1b8505a62b8c124c5d"
}

resource "tls_private_key" "experiment" {
  algorithm = "RSA"
  rsa_bits  = 2048
}


resource "aws_security_group" "sg-eu_central_1" {
  provider        = aws.eu_central_1
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

resource "aws_key_pair" "key-eu_central_1" {
  provider        = aws.eu_central_1
  key_name   = "my-terraform-key"
  public_key = tls_private_key.experiment.public_key_openssh
}

resource "aws_instance" "instance-eu_central_1" {
  provider        = aws.eu_central_1
  ami             = "ami-0faab6bdbac9486fb"
  instance_type   = "t2.micro"
  key_name = aws_key_pair.key-eu_central_1.key_name
  vpc_security_group_ids = [aws_security_group.sg-eu_central_1.id]

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


resource "null_resource" "prov-eu_central_1" {
  depends_on = [aws_instance.instance-eu_central_1, aws_instance.instance-us_east_1, aws_instance.instance-eu_west_1]

  provisioner "remote-exec" {
    connection {
      host        = aws_instance.instance-eu_central_1.public_ip
      user        = "ubuntu"
      private_key = tls_private_key.experiment.private_key_pem
    }

    inline = [
      "sleep 30 && /tmp/binary --id 0 --message-size 1000 --message-rate 10 --port 4100 --peers-addrs ${aws_instance.instance-us_east_1.public_ip}:4100,${aws_instance.instance-eu_west_1.public_ip}:4100",
    ]
  }
}

resource "aws_security_group" "sg-us_east_1" {
  provider        = aws.us_east_1
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

resource "aws_key_pair" "key-us_east_1" {
  provider        = aws.eu_central_1
  key_name   = "my-terraform-key"
  public_key = tls_private_key.experiment.public_key_openssh
}

resource "aws_instance" "instance-us_east_1" {
  provider        = aws.us_east_1
  ami             = "ami-0c7217cdde317cfec"
  instance_type   = "t2.micro"
  key_name = aws_key_pair.key-us_east_1.key_name
  vpc_security_group_ids = [aws_security_group.sg-us_east_1.id]

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


resource "null_resource" "prov-us_east_1" {
  depends_on = [aws_instance.instance-eu_central_1, aws_instance.instance-us_east_1, aws_instance.instance-eu_west_1]

  provisioner "remote-exec" {
    connection {
      host        = aws_instance.instance-us_east_1.public_ip
      user        = "ubuntu"
      private_key = tls_private_key.experiment.private_key_pem
    }

    inline = [
      "sleep 30 && /tmp/binary --id 1 --message-size 1000 --message-rate 10 --port 4100 --peers-addrs ${aws_instance.instance-eu_central_1.public_ip}:4100,${aws_instance.instance-eu_west_1.public_ip}:4100",
    ]
  }
}

resource "aws_security_group" "sg-eu_west_1" {
  provider        = aws.eu_west_1
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

resource "aws_key_pair" "key-eu_west_1" {
  provider        = aws.eu_central_1
  key_name   = "my-terraform-key"
  public_key = tls_private_key.experiment.public_key_openssh
}

resource "aws_instance" "instance-eu_west_1" {
  provider        = aws.eu_west_1
  ami             = "ami-0905a3c97561e0b69"
  instance_type   = "t2.micro"
  key_name = aws_key_pair.key-eu_west_1.key_name
  vpc_security_group_ids = [aws_security_group.sg-eu_west_1.id]

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


resource "null_resource" "prov-eu_west_1" {
  depends_on = [aws_instance.instance-eu_central_1, aws_instance.instance-us_east_1, aws_instance.instance-eu_west_1]

  provisioner "remote-exec" {
    connection {
      host        = aws_instance.instance-eu_west_1.public_ip
      user        = "ubuntu"
      private_key = tls_private_key.experiment.private_key_pem
    }

    inline = [
      "sleep 30 && /tmp/binary --id 2 --message-size 1000 --message-rate 10 --port 4100 --peers-addrs ${aws_instance.instance-eu_central_1.public_ip}:4100,${aws_instance.instance-us_east_1.public_ip}:4100",
    ]
  }
}
