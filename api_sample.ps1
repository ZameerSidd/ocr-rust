# Define the URI with the required query parameter
$Uri = "http://15.15.15.29:8791/ocr/deepseek-ocr?counter_name=ATM"

# Read your image file as a byte array (corresponds to 'body: Bytes' in Rust)
$FilePath = "C:\Users\AhmeZam\Pictures\Screenshots\sample_1.png"
$FileBytes = [System.IO.File]::ReadAllBytes($FilePath)

# Execute the POST request
$Response = Invoke-RestMethod -Uri $Uri `
                             -Method Post `
                             -Body $FileBytes `
                             -ContentType "application/octet-stream"

# View the output
$Response
