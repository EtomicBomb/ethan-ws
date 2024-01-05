sudo certbot -d ethan.ws,www.ethan.ws --manual --preferred-challenges dns certonly
cp /etc/letsencrypt/live/ethan.ws/fullchain.pem ~/secret/cert.pem
cp /etc/letsencrypt/live/ethan.ws/privkey.pem ~/secret/key.rsa
sudo chown ubuntu ~/secret/*
sudo setcap CAP_NET_BIND_SERVICE=+eip server
pkill server
nohup ~/server &
# https://certbot.eff.org/instructions?ws=other&os=ubuntufocal&tab=standard
