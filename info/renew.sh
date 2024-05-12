# HOSTNAME: '_acme-challenge'
# HOSTNAME: '_acme-challenge.www'

cd ~/ethan-ws
sudo certbot -d ethan.ws,www.ethan.ws --manual --preferred-challenges dns certonly
cp /etc/letsencrypt/live/ethan.ws/fullchain.pem secret/cert.pem
cp /etc/letsencrypt/live/ethan.ws/privkey.pem secret/key.pem
sudo chown ubuntu secret/*
sudo setcap CAP_NET_BIND_SERVICE=+eip target/debug/ethan-ws
pkill ethan-ws
nohup target/debug/ethan-ws &
# https://certbot.eff.org/instructions?ws=other&os=ubuntufocal&tab=standard

