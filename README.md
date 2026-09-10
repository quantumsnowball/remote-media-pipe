# remote-media-pipe
A remote media serving application written in Rust

# Usage
./remote-media-pipe local ~/Movies 192.168.1.50:7879
./remote-media-pipe sftp s7:~/Movies 192.168.1.50:7879
./remote-media-pipe gdrive qsc:DLNA/ 192.168.1.50:7879
./remote-media-pipe sftp --help
