import requests


create_user_response = requests.post('https://trekkie.staging.dvb.solutions/user/create')

print(create_user_response)
