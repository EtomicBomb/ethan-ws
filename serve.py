import http.server

port = 8000
directory = 'front'

class MyHandler(
    http.server.SimpleHTTPRequestHandler
):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=directory, **kwargs)
    def send_response_only(self, code, message=None):
        super().send_response_only(code, message)
        self.send_header('Cache-Control', 'no-store, must-revalidate')
        self.send_header('Expires', '0')

if __name__ == '__main__':
    http.server.test(HandlerClass=MyHandler, port=port)

