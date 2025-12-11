PI controller to control mac on linux fans
ive only tested it on my 2014 mbp running arch

I wrote this because i found mbpfan to change fan speeds too quickly and kinda for fun 

the config file contains the path to the smc, which should be kinda standard, the initial integral value (not super important), the weighting of the integral term (quite important), the weighting of the proportional term (quite important) and the target value (very important)

runit shows all you need for the build process
